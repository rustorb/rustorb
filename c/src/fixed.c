/* The fixed functions of the C mapping (1.14).

   The values are CORBA_fixed_d_s: _digits, _scale and the packed decimal
   of the CDR (two digits an octet, most significant first, the last
   half-octet the sign, 0xC or 0xD). The arithmetic is exact on decimal
   digits; the result is then made to fit the _digits and _scale of *rp:
   digits below its scale are truncated (toward zero, as the mapping
   allows), and digits above its _digits are dropped. */

#include "internal.h"

#define MAX_DIGITS 128

typedef struct dec {
  int neg;
  int scale;
  /* The digits, least significant first; n of them, no leading zeros. */
  int n;
  unsigned char d[MAX_DIGITS];
} dec;

typedef struct fixed_head {
  CORBA_unsigned_short _digits;
  CORBA_short _scale;
  CORBA_octet _value[1];
} fixed_head;

static void trim(dec* x) {
  while (x->n > 0 && x->d[x->n - 1] == 0) x->n--;
  if (x->n == 0) x->neg = 0;
}

static int nibble(const CORBA_octet* v, int k) {
  return k % 2 == 0 ? v[k / 2] >> 4 : v[k / 2] & 0x0f;
}

static void decode(const void* fp, dec* x) {
  const fixed_head* f = (const fixed_head*)fp;
  int digits = f->_digits > 31 ? 31 : f->_digits;
  int total = 2 * ((digits + 2) / 2);
  int i;
  memset(x, 0, sizeof *x);
  x->scale = f->_scale;
  for (i = 0; i < digits; i++) {
    int v = nibble(f->_value, total - 2 - i);
    x->d[i] = (unsigned char)(v > 9 ? 0 : v);
  }
  x->n = digits;
  x->neg = nibble(f->_value, total - 1) == 0x0d;
  trim(x);
}

/* x times 10^k (k >= 0), or x divided by 10^-k truncated. */
static void shift(dec* x, int k) {
  if (k > 0) {
    if (x->n == 0) return;
    if (x->n + k > MAX_DIGITS) k = MAX_DIGITS - x->n;
    memmove(x->d + k, x->d, (size_t)x->n);
    memset(x->d, 0, (size_t)k);
    x->n += k;
  } else if (k < 0) {
    k = -k;
    if (k >= x->n) {
      x->n = 0;
    } else {
      memmove(x->d, x->d + k, (size_t)(x->n - k));
      x->n -= k;
    }
  }
  trim(x);
}

static void rescale(dec* x, int scale) {
  shift(x, scale - x->scale);
  x->scale = scale;
}

static void encode(void* rp, dec x) {
  fixed_head* f = (fixed_head*)rp;
  int digits = f->_digits > 31 ? 31 : f->_digits;
  int octets = (digits + 2) / 2;
  int total = 2 * octets;
  int i;
  rescale(&x, f->_scale);
  if (x.n > digits) {
    x.n = digits;
    trim(&x);
  }
  memset(f->_value, 0, (size_t)octets);
  for (i = 0; i < x.n; i++) {
    int k = total - 2 - i;
    f->_value[k / 2] |= (CORBA_octet)(k % 2 == 0 ? x.d[i] << 4 : x.d[i]);
  }
  f->_value[octets - 1] |= x.neg ? 0x0d : 0x0c;
}

static int compare_mag(const dec* a, const dec* b) {
  int i;
  if (a->n != b->n) return a->n < b->n ? -1 : 1;
  for (i = a->n - 1; i >= 0; i--)
    if (a->d[i] != b->d[i]) return a->d[i] < b->d[i] ? -1 : 1;
  return 0;
}

/* |a| + |b| into r. */
static void add_mag(const dec* a, const dec* b, dec* r) {
  int i, carry = 0, n = a->n > b->n ? a->n : b->n;
  for (i = 0; i < n && i < MAX_DIGITS; i++) {
    int s = (i < a->n ? a->d[i] : 0) + (i < b->n ? b->d[i] : 0) + carry;
    r->d[i] = (unsigned char)(s % 10);
    carry = s / 10;
  }
  if (carry && i < MAX_DIGITS) r->d[i++] = (unsigned char)carry;
  r->n = i;
}

/* |a| - |b| into r, |a| >= |b|. */
static void sub_mag(const dec* a, const dec* b, dec* r) {
  int i, borrow = 0;
  for (i = 0; i < a->n; i++) {
    int s = a->d[i] - (i < b->n ? b->d[i] : 0) - borrow;
    borrow = s < 0;
    r->d[i] = (unsigned char)(s < 0 ? s + 10 : s);
  }
  r->n = a->n;
}

static dec add(dec a, dec b, int negate_b) {
  dec r;
  int scale = a.scale > b.scale ? a.scale : b.scale;
  memset(&r, 0, sizeof r);
  rescale(&a, scale);
  rescale(&b, scale);
  if (negate_b && b.n) b.neg = !b.neg;
  r.scale = scale;
  if (a.neg == b.neg) {
    add_mag(&a, &b, &r);
    r.neg = a.neg;
  } else if (compare_mag(&a, &b) >= 0) {
    sub_mag(&a, &b, &r);
    r.neg = a.neg;
  } else {
    sub_mag(&b, &a, &r);
    r.neg = b.neg;
  }
  trim(&r);
  return r;
}

static dec mul(const dec* a, const dec* b) {
  dec r;
  int i, j, acc[2 * MAX_DIGITS];
  memset(&r, 0, sizeof r);
  memset(acc, 0, sizeof acc);
  for (i = 0; i < a->n; i++)
    for (j = 0; j < b->n; j++) acc[i + j] += a->d[i] * b->d[j];
  r.n = a->n + b->n;
  for (i = 0; i < r.n; i++) {
    if (i + 1 < 2 * MAX_DIGITS) acc[i + 1] += acc[i] / 10;
    acc[i] %= 10;
  }
  if (r.n > MAX_DIGITS) r.n = MAX_DIGITS; /* never: the operands have 31 digits at most */
  for (i = 0; i < r.n; i++) r.d[i] = (unsigned char)acc[i];
  r.scale = a->scale + b->scale;
  r.neg = a->neg != b->neg;
  trim(&r);
  return r;
}

/* a / b with the given scale, truncated; 0 when b is 0. */
static dec divide(dec a, const dec* b, int scale) {
  dec q, rem, den = *b;
  int i;
  memset(&q, 0, sizeof q);
  q.scale = scale;
  if (b->n == 0) return q;
  /* a / b at scale s is (a * 10^(s - sa + sb)) / b on the digits. */
  den.neg = 0;
  den.scale = 0;
  shift(&a, scale - a.scale + b->scale);
  memset(&rem, 0, sizeof rem);
  q.n = a.n;
  for (i = a.n - 1; i >= 0; i--) {
    int count = 0;
    shift(&rem, 1);
    if (rem.n == 0 && a.d[i]) rem.n = 1;
    rem.d[0] = a.d[i];
    trim(&rem);
    while (compare_mag(&rem, &den) >= 0) {
      dec t;
      memset(&t, 0, sizeof t);
      sub_mag(&rem, &den, &t);
      trim(&t);
      rem = t;
      count++;
    }
    q.d[i] = (unsigned char)count;
  }
  q.neg = a.neg != b->neg;
  trim(&q);
  return q;
}

static CORBA_long to_long(const dec* x) {
  long long v = 0;
  int i;
  for (i = x->n - 1; i >= 0; i--) {
    v = v * 10 + x->d[i];
    if (v > 2147483647LL + 1) break;
  }
  if (x->neg) return v > 2147483648LL ? (CORBA_long)(-2147483647 - 1) : (CORBA_long)-v;
  return v > 2147483647LL ? 2147483647 : (CORBA_long)v;
}

static dec of_long(CORBA_long v, int scale) {
  dec x;
  long long m = v;
  memset(&x, 0, sizeof x);
  x.scale = scale;
  if (m < 0) {
    x.neg = 1;
    m = -m;
  }
  while (m) {
    x.d[x.n++] = (unsigned char)(m % 10);
    m /= 10;
  }
  return x;
}

/* The digits fraction_part and set take: the scale, at most 9. */
static int fraction_digits(int scale) {
  return scale < 0 ? 0 : (scale > 9 ? 9 : scale);
}

CORBA_long CORBA_fixed_integer_part(const void* fp) {
  dec x;
  if (!fp) return 0;
  decode(fp, &x);
  rescale(&x, 0);
  return to_long(&x);
}

CORBA_long CORBA_fixed_fraction_part(const void* fp) {
  dec x, i;
  int k;
  if (!fp) return 0;
  decode(fp, &x);
  k = fraction_digits(x.scale);
  i = x;
  rescale(&i, 0);
  x = add(x, i, 1);
  rescale(&x, k);
  return to_long(&x);
}

void CORBA_fixed_set(void* rp, const CORBA_long i, const CORBA_long f) {
  int scale;
  if (!rp) return;
  scale = ((fixed_head*)rp)->_scale;
  encode(rp, add(of_long(i, 0), of_long(f, fraction_digits(scale)), 0));
}

void CORBA_fixed_add(void* rp, const void* f1p, const void* f2p) {
  dec a, b;
  if (!rp || !f1p || !f2p) return;
  decode(f1p, &a);
  decode(f2p, &b);
  encode(rp, add(a, b, 0));
}

void CORBA_fixed_sub(void* rp, const void* f1p, const void* f2p) {
  dec a, b;
  if (!rp || !f1p || !f2p) return;
  decode(f1p, &a);
  decode(f2p, &b);
  encode(rp, add(a, b, 1));
}

void CORBA_fixed_mul(void* rp, const void* f1p, const void* f2p) {
  dec a, b;
  if (!rp || !f1p || !f2p) return;
  decode(f1p, &a);
  decode(f2p, &b);
  encode(rp, mul(&a, &b));
}

void CORBA_fixed_div(void* rp, const void* f1p, const void* f2p) {
  dec a, b;
  if (!rp || !f1p || !f2p) return;
  decode(f1p, &a);
  decode(f2p, &b);
  encode(rp, divide(a, &b, ((fixed_head*)rp)->_scale));
}

CORBA_fixed_d_s* CORBA_fixed_alloc(CORBA_unsigned_short d) {
  CORBA_fixed_d_s* f;
  if (d > 31) return NULL;
  f = (CORBA_fixed_d_s*)rustorb_c_alloc(NULL, sizeof *f);
  if (!f) return NULL;
  f->_digits = d;
  f->_scale = 0;
  f->_value[(d + 2) / 2 - 1] = 0x0c;
  return f;
}
