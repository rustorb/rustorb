// Generates the stubs and skeletons of Echo.idl into $OUT_DIR/echo.rs with
// rustorb-idl -brust (rustorb-idl must be on PATH, or set RUSTORB_IDL).
fn main() {
    rustorb_codegen::Config::new()
        .compile(&["../idl/Echo.idl"])
        .unwrap();
}
