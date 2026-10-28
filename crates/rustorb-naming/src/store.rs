//! Where the naming service keeps its bindings: the [`Store`] trait, the
//! in-memory [`MemoryStore`] and the default [`LogStore`] (an append-only
//! log and checkpoints).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rustorb_core::name::{self, NameComponent};

use crate::BindingType;

/// One binding as stored: what it is bound to, and the stringified IOR of
/// the object or context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBinding {
    /// Object or context.
    pub binding_type: BindingType,
    /// The stringified IOR (`IOR:...`); a nil reference is stored as the
    /// nil IOR.
    pub ior: String,
}

/// The whole state of a naming service: its contexts by id, and the
/// bindings of each context by name component.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// The contexts and their bindings.
    pub contexts: BTreeMap<String, BTreeMap<NameComponent, StoredBinding>>,
}

/// One update of a [`Snapshot`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// A context was created (`new_context`, `bind_new_context`).
    CreateContext(String),
    /// A context was destroyed.
    DestroyContext(String),
    /// A name was bound or rebound in a context.
    Bind {
        /// The context id.
        context: String,
        /// The name in the context.
        component: NameComponent,
        /// What it is bound to.
        binding: StoredBinding,
    },
    /// A name was unbound from a context.
    Unbind {
        /// The context id.
        context: String,
        /// The name in the context.
        component: NameComponent,
    },
}

impl Snapshot {
    /// Applies `change`. A change is applied the same way whatever the
    /// state, so replaying a log over a newer checkpoint gives the same
    /// result: creating a context that exists keeps its bindings, binding
    /// in a context that does not exist creates it, and unbinding or
    /// destroying what does not exist does nothing.
    pub fn apply(&mut self, change: &Change) {
        match change {
            Change::CreateContext(id) => {
                self.contexts.entry(id.clone()).or_default();
            }
            Change::DestroyContext(id) => {
                self.contexts.remove(id);
            }
            Change::Bind {
                context,
                component,
                binding,
            } => {
                self.contexts
                    .entry(context.clone())
                    .or_default()
                    .insert(component.clone(), binding.clone());
            }
            Change::Unbind { context, component } => {
                if let Some(bindings) = self.contexts.get_mut(context) {
                    bindings.remove(component);
                }
            }
        }
    }

    /// The changes that build this snapshot from an empty one.
    pub fn changes(&self) -> impl Iterator<Item = Change> + '_ {
        let creates = self
            .contexts
            .keys()
            .map(|id| Change::CreateContext(id.clone()));
        let binds = self.contexts.iter().flat_map(|(id, bindings)| {
            bindings
                .iter()
                .map(move |(component, binding)| Change::Bind {
                    context: id.clone(),
                    component: component.clone(),
                    binding: binding.clone(),
                })
        });
        creates.chain(binds)
    }
}

/// The storage of a naming service. The service calls
/// [`record`](Store::record) for every update before it applies it and
/// replies, and offers a [`checkpoint`](Store::checkpoint) after each
/// update.
pub trait Store: Send {
    /// The state to start from. Called once, before anything else.
    fn load(&mut self) -> io::Result<Snapshot>;

    /// Records `change`. When this returns, the change must survive a
    /// crash; an error fails the operation with `PERSIST_STORE`.
    fn record(&mut self, change: &Change) -> io::Result<()>;

    /// Called after each update with the state that includes it, so that
    /// the store can write a checkpoint when it sees fit.
    fn checkpoint(&mut self, state: &Snapshot) -> io::Result<()> {
        let _ = state;
        Ok(())
    }
}

/// A store that keeps nothing: the service starts empty every time.
#[derive(Debug, Default)]
pub struct MemoryStore;

impl Store for MemoryStore {
    fn load(&mut self) -> io::Result<Snapshot> {
        Ok(Snapshot::default())
    }

    fn record(&mut self, _change: &Change) -> io::Result<()> {
        Ok(())
    }
}

/// The first word of the header line of both files.
const MAGIC: &str = "rustorb-naming";
/// The format version, the second word of the header line.
const FORMAT: u32 = 1;
const CHECKPOINT_FILE: &str = "naming.ckp";
const LOG_FILE: &str = "naming.log";

/// The default store: every update is appended to a log file and synced
/// before the service replies, and every so often the whole state is
/// written as a checkpoint and the log emptied.
///
/// The directory holds `naming.ckp`, the last checkpoint, and
/// `naming.log`, the updates since. Both are text with one record per
/// line, starting with a header `rustorb-naming 1 GENERATION`:
///
/// ```text
/// rustorb-naming 1 3
/// context NameService
/// bind NameService o app.kind IOR:0100...
/// bind NameService c sub IOR:0100...
/// unbind NameService app.kind
/// destroy 5f3a9c0e12ab44d1
/// ```
///
/// A name component is written in the stringified name form
/// (`id.kind`, with `\` escapes), and `%`, spaces and control characters
/// in it as `%XX`. A checkpoint ends with an `end` line.
///
/// A checkpoint is written to a temporary file, synced and renamed over
/// `naming.ckp`; then the log is started again with the checkpoint's
/// generation. At load, a log of an older generation than the checkpoint
/// (a crash between the two steps) is ignored, and a last line without
/// its newline (a crash while appending) is dropped.
#[derive(Debug)]
pub struct LogStore {
    dir: PathBuf,
    generation: u64,
    log: Option<BufWriter<File>>,
    records: usize,
    last_checkpoint: Instant,
    checkpoint_records: usize,
    checkpoint_interval: Option<Duration>,
}

impl LogStore {
    /// A store in the directory `dir`, created if needed. A checkpoint is
    /// written every 1000 updates.
    pub fn new(dir: impl Into<PathBuf>) -> LogStore {
        LogStore {
            dir: dir.into(),
            generation: 0,
            log: None,
            records: 0,
            last_checkpoint: Instant::now(),
            checkpoint_records: 1000,
            checkpoint_interval: None,
        }
    }

    /// Writes a checkpoint after `records` updates (at least 1).
    pub fn checkpoint_every(mut self, records: usize) -> LogStore {
        self.checkpoint_records = records.max(1);
        self
    }

    /// Also writes a checkpoint at the first update after `interval` has
    /// passed since the last one.
    pub fn checkpoint_interval(mut self, interval: Duration) -> LogStore {
        self.checkpoint_interval = Some(interval);
        self
    }

    /// The directory of the files.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, file: &str) -> PathBuf {
        self.dir.join(file)
    }

    /// Writes `state` as the checkpoint of the next generation and starts
    /// an empty log for it.
    fn write_checkpoint(&mut self, state: &Snapshot) -> io::Result<()> {
        let generation = self.generation + 1;
        let tmp = self.path("naming.ckp.tmp");
        {
            let mut out = BufWriter::new(File::create(&tmp)?);
            writeln!(out, "{MAGIC} {FORMAT} {generation}")?;
            for change in state.changes() {
                out.write_all(format_change(&change).as_bytes())?;
            }
            writeln!(out, "end")?;
            out.into_inner().map_err(|e| e.into_error())?.sync_all()?;
        }
        fs::rename(&tmp, self.path(CHECKPOINT_FILE))?;
        sync_dir(&self.dir);
        self.generation = generation;
        self.start_log()?;
        self.records = 0;
        self.last_checkpoint = Instant::now();
        Ok(())
    }

    fn start_log(&mut self) -> io::Result<()> {
        self.log = None;
        let mut file = File::create(self.path(LOG_FILE))?;
        writeln!(file, "{MAGIC} {FORMAT} {}", self.generation)?;
        file.sync_all()?;
        sync_dir(&self.dir);
        self.log = Some(BufWriter::new(file));
        Ok(())
    }
}

impl Store for LogStore {
    fn load(&mut self) -> io::Result<Snapshot> {
        fs::create_dir_all(&self.dir)?;
        let mut state = Snapshot::default();
        let mut generation = 0;
        if let Some(text) = read_optional(&self.path(CHECKPOINT_FILE))? {
            let (header, lines) = parse_file(&text, CHECKPOINT_FILE)?;
            generation = header;
            let mut ended = false;
            for (n, line) in lines {
                if ended {
                    return Err(bad_data(CHECKPOINT_FILE, n, "record after `end`"));
                }
                if line == "end" {
                    ended = true;
                    continue;
                }
                state.apply(&parse_change(line).ok_or_else(|| bad_record(CHECKPOINT_FILE, n))?);
            }
            if !ended {
                return Err(bad_data(CHECKPOINT_FILE, 0, "no `end` line"));
            }
        }
        let mut replayed = 0;
        if let Some(text) = read_optional(&self.path(LOG_FILE))? {
            // A crash while appending leaves a last line without its
            // newline: that update was never acknowledged.
            let complete = match text.rfind('\n') {
                Some(end) => &text[..=end],
                None => "",
            };
            if !complete.is_empty() {
                let (log_generation, lines) = parse_file(complete, LOG_FILE)?;
                if log_generation > generation {
                    return Err(bad_data(LOG_FILE, 1, "newer than the checkpoint"));
                }
                if log_generation == generation {
                    for (n, line) in lines {
                        state.apply(&parse_change(line).ok_or_else(|| bad_record(LOG_FILE, n))?);
                        replayed += 1;
                    }
                }
            }
        }
        self.generation = generation;
        if replayed > 0 || generation == 0 {
            self.write_checkpoint(&state)?;
        } else {
            let file = OpenOptions::new()
                .append(true)
                .open(self.path(LOG_FILE))
                .or_else(|_| {
                    self.start_log()?;
                    OpenOptions::new().append(true).open(self.path(LOG_FILE))
                })?;
            self.log = Some(BufWriter::new(file));
        }
        Ok(state)
    }

    fn record(&mut self, change: &Change) -> io::Result<()> {
        let log = self
            .log
            .as_mut()
            .ok_or_else(|| io::Error::other("the log store was not loaded"))?;
        log.write_all(format_change(change).as_bytes())?;
        log.flush()?;
        log.get_ref().sync_data()?;
        self.records += 1;
        Ok(())
    }

    fn checkpoint(&mut self, state: &Snapshot) -> io::Result<()> {
        let due = self.records >= self.checkpoint_records
            || self
                .checkpoint_interval
                .is_some_and(|i| self.records > 0 && self.last_checkpoint.elapsed() >= i);
        if due {
            self.write_checkpoint(state)?;
        }
        Ok(())
    }
}

fn read_optional(path: &Path) -> io::Result<Option<String>> {
    match File::open(path) {
        Ok(mut file) => {
            let mut text = String::new();
            file.read_to_string(&mut text)?;
            Ok(Some(text))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Makes a rename or a new file in `dir` durable. Not every platform can
/// open a directory, so failures are ignored.
fn sync_dir(dir: &Path) {
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
}

/// The generation in the header, and the numbered lines after it.
fn parse_file<'a>(
    text: &'a str,
    file: &str,
) -> io::Result<(u64, impl Iterator<Item = (usize, &'a str)>)> {
    let mut lines = text.lines().enumerate().map(|(i, l)| (i + 1, l));
    let header = lines.next().map(|(_, l)| l).unwrap_or("");
    let mut words = header.split(' ');
    if words.next() != Some(MAGIC) {
        return Err(bad_data(file, 1, "not a rustorb naming file"));
    }
    if words.next() != Some(&FORMAT.to_string()) {
        return Err(bad_data(file, 1, "unknown format version"));
    }
    let generation = words
        .next()
        .and_then(|g| g.parse().ok())
        .ok_or_else(|| bad_data(file, 1, "bad generation"))?;
    Ok((generation, lines))
}

fn bad_data(file: &str, line: usize, what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("{file}:{line}: {what}"))
}

fn bad_record(file: &str, line: usize) -> io::Error {
    bad_data(file, line, "malformed record")
}

/// A change as one line of the log, newline included.
fn format_change(change: &Change) -> String {
    match change {
        Change::CreateContext(id) => format!("context {id}\n"),
        Change::DestroyContext(id) => format!("destroy {id}\n"),
        Change::Bind {
            context,
            component,
            binding,
        } => {
            let t = match binding.binding_type {
                BindingType::Object => 'o',
                BindingType::Context => 'c',
            };
            format!(
                "bind {context} {t} {} {}\n",
                format_component(component),
                binding.ior
            )
        }
        Change::Unbind { context, component } => {
            format!("unbind {context} {}\n", format_component(component))
        }
    }
}

fn parse_change(line: &str) -> Option<Change> {
    let mut words = line.split(' ');
    let change = match words.next()? {
        "context" => Change::CreateContext(id(words.next()?)?),
        "destroy" => Change::DestroyContext(id(words.next()?)?),
        "bind" => {
            let context = id(words.next()?)?;
            let binding_type = match words.next()? {
                "o" => BindingType::Object,
                "c" => BindingType::Context,
                _ => return None,
            };
            let component = parse_component(words.next()?)?;
            let ior = words.next()?.to_owned();
            if !ior.starts_with("IOR:") {
                return None;
            }
            Change::Bind {
                context,
                component,
                binding: StoredBinding { binding_type, ior },
            }
        }
        "unbind" => Change::Unbind {
            context: id(words.next()?)?,
            component: parse_component(words.next()?)?,
        },
        _ => return None,
    };
    words.next().is_none().then_some(change)
}

fn id(word: &str) -> Option<String> {
    (!word.is_empty()).then(|| word.to_owned())
}

/// A name component in the stringified name form, with `%`, spaces and
/// control characters escaped as `%XX`.
fn format_component(component: &NameComponent) -> String {
    let s = name::to_string(std::slice::from_ref(component)).expect("one component is a name");
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '%' || c == ' ' || c.is_control() {
            let mut buf = [0; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                let _ = write!(out, "%{b:02X}");
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn parse_component(word: &str) -> Option<NameComponent> {
    let mut bytes = Vec::with_capacity(word.len());
    let mut rest = word.as_bytes();
    while let Some((&b, tail)) = rest.split_first() {
        if b == b'%' {
            let hex = std::str::from_utf8(tail.get(..2)?).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            rest = &tail[2..];
        } else {
            bytes.push(b);
            rest = tail;
        }
    }
    let mut name = name::to_name(&String::from_utf8(bytes).ok()?).ok()?;
    (name.len() == 1).then(|| name.remove(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nc(id: &str, kind: &str) -> NameComponent {
        NameComponent::new(id, kind)
    }

    fn bind(context: &str, component: NameComponent, ior: &str) -> Change {
        Change::Bind {
            context: context.into(),
            component,
            binding: StoredBinding {
                binding_type: BindingType::Object,
                ior: ior.into(),
            },
        }
    }

    #[test]
    fn records_read_back() {
        let changes = [
            Change::CreateContext("NameService".into()),
            Change::DestroyContext("0123abcd".into()),
            bind("NameService", nc("a b%c", "k\nd"), "IOR:00"),
            bind("NameService", nc("", ""), "IOR:01"),
            bind("x", nc("dots.and/slash\\", "日本"), "IOR:02"),
            Change::Unbind {
                context: "x".into(),
                component: nc("a", ""),
            },
        ];
        for change in changes {
            let line = format_change(&change);
            assert_eq!(line.matches('\n').count(), 1, "{line:?}");
            assert_eq!(parse_change(line.trim_end_matches('\n')), Some(change));
        }
        assert_eq!(parse_change("bind x o a IOR:00 extra"), None);
        assert_eq!(parse_change("bind x z a IOR:00"), None);
        assert_eq!(parse_change("unbind x a//b"), None);
        assert_eq!(parse_change("frobnicate x"), None);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rustorb-naming-store-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn update(store: &mut LogStore, state: &mut Snapshot, change: Change) {
        store.record(&change).unwrap();
        state.apply(&change);
        store.checkpoint(state).unwrap();
    }

    #[test]
    fn log_and_checkpoints_survive_a_restart() {
        let dir = temp_dir("restart");
        let mut store = LogStore::new(&dir).checkpoint_every(3);
        let mut state = store.load().unwrap();
        assert_eq!(state, Snapshot::default());
        update(&mut store, &mut state, Change::CreateContext("root".into()));
        for i in 0..4 {
            update(
                &mut store,
                &mut state,
                bind("root", nc(&format!("n{i}"), ""), "IOR:00"),
            );
        }
        update(
            &mut store,
            &mut state,
            Change::Unbind {
                context: "root".into(),
                component: nc("n1", ""),
            },
        );
        drop(store);

        let mut again = LogStore::new(&dir);
        assert_eq!(again.load().unwrap(), state);
        // Loading folded the log into a checkpoint.
        let log = fs::read_to_string(dir.join(LOG_FILE)).unwrap();
        assert_eq!(log.lines().count(), 1, "{log}");
        update(
            &mut again,
            &mut state,
            Change::DestroyContext("root".into()),
        );
        drop(again);
        assert_eq!(LogStore::new(&dir).load().unwrap(), state);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_torn_last_record_is_dropped() {
        let dir = temp_dir("torn");
        let mut store = LogStore::new(&dir);
        let mut state = store.load().unwrap();
        update(&mut store, &mut state, Change::CreateContext("root".into()));
        drop(store);
        let mut log = OpenOptions::new()
            .append(true)
            .open(dir.join(LOG_FILE))
            .unwrap();
        log.write_all(b"bind root o half IOR:0").unwrap();
        drop(log);
        assert_eq!(LogStore::new(&dir).load().unwrap(), state);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_log_older_than_the_checkpoint_is_ignored() {
        let dir = temp_dir("stale");
        let mut store = LogStore::new(&dir);
        let mut state = store.load().unwrap();
        update(&mut store, &mut state, Change::CreateContext("root".into()));
        update(&mut store, &mut state, bind("root", nc("a", ""), "IOR:00"));
        let stale_log = fs::read_to_string(dir.join(LOG_FILE)).unwrap();
        // A checkpoint is written, but the crash comes before the log is
        // started again.
        update(
            &mut store,
            &mut state,
            Change::Unbind {
                context: "root".into(),
                component: nc("a", ""),
            },
        );
        store.write_checkpoint(&state).unwrap();
        drop(store);
        fs::write(dir.join(LOG_FILE), stale_log).unwrap();
        assert_eq!(LogStore::new(&dir).load().unwrap(), state);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_damaged_file_is_an_error() {
        let dir = temp_dir("damaged");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(CHECKPOINT_FILE), "rustorb-naming 1 1\ncontext a\n").unwrap();
        let err = LogStore::new(&dir).load().unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        fs::write(dir.join(CHECKPOINT_FILE), "rustorb-naming 1 1\nend\n").unwrap();
        fs::write(dir.join(LOG_FILE), "rustorb-naming 1 1\nnonsense\n").unwrap();
        let err = LogStore::new(&dir).load().unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        fs::remove_dir_all(&dir).unwrap();
    }
}
