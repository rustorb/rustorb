/* The fixed functions of the C mapping (1.14).
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>
#include <string.h>

#include <rustorb/orb.h>

static int failures;

#define CHECK(cond)                                                    \
  do {                                                                 \
    if (!(cond)) {                                                     \
      failures++;                                                      \
      fprintf(stderr, "%s:%d: CHECK(%s) failed\n", __FILE__, __LINE__, #cond); \
    }                                                                  \
  } while (0)

RUSTORB_C_FIXED(9, 2);
RUSTORB_C_FIXED(15, 5);
RUSTORB_C_FIXED(4, 0);
RUSTORB_C_FIXED(31, 0);
RUSTORB_C_FIXED(5, 3);

#define INIT(f, d, s) ((f)._digits = (d), (f)._scale = (s))

/* The value as text, digits and sign from the packed decimal. */
static void text(const void* fp, char* out) {
  const CORBA_fixed_d_s* f = (const CORBA_fixed_d_s*)fp;
  int total = 2 * ((f->_digits + 2) / 2), i, k = 0, started = 0;
  int sign = f->_value[total / 2 - 1] & 0x0f;
  int intdigits = f->_digits - f->_scale;
  if (sign == 0x0d) out[k++] = '-';
  for (i = 0; i < f->_digits; i++) {
    int pos = total - 1 - f->_digits + i;
    int v = pos % 2 == 0 ? f->_value[pos / 2] >> 4 : f->_value[pos / 2] & 0x0f;
    if (i == intdigits) {
      if (!started) out[k++] = '0';
      out[k++] = '.';
      started = 1;
    }
    if (v || started || i == intdigits - 1) {
      out[k++] = (char)('0' + v);
      started = 1;
    }
  }
  if (!started) out[k++] = '0';
  out[k] = 0;
}

#define IS(f, s)                                                                      \
  do {                                                                                \
    char buf_[64];                                                                    \
    text(&(f), buf_);                                                                 \
    if (strcmp(buf_, (s)) != 0) {                                                     \
      failures++;                                                                     \
      fprintf(stderr, "%s:%d: %s is %s, not %s\n", __FILE__, __LINE__, #f, buf_, (s)); \
    }                                                                                 \
  } while (0)

int main(void) {
  CORBA_fixed_9_2 a, b, r;
  CORBA_fixed_15_5 w;
  CORBA_fixed_4_0 small;
  CORBA_fixed_31_0 big, big2;
  CORBA_fixed_5_3 f53;
  CORBA_fixed_d_s* any;

  INIT(a, 9, 2);
  INIT(b, 9, 2);
  INIT(r, 9, 2);
  INIT(w, 15, 5);
  INIT(small, 4, 0);
  INIT(big, 31, 0);
  INIT(big2, 31, 0);
  INIT(f53, 5, 3);

  CORBA_fixed_set(&a, 123, 45);
  IS(a, "123.45");
  CHECK(CORBA_fixed_integer_part(&a) == 123 && CORBA_fixed_fraction_part(&a) == 45);
  /* The CDR form: 0000123.45 in 5 octets, sign 0xC. */
  CHECK(a._value[0] == 0x00 && a._value[1] == 0x00 && a._value[2] == 0x12 &&
        a._value[3] == 0x34 && a._value[4] == 0x5c);
  CORBA_fixed_set(&b, -7, -5);
  IS(b, "-7.05");
  CHECK(b._value[4] == 0x5d);
  CHECK(CORBA_fixed_integer_part(&b) == -7 && CORBA_fixed_fraction_part(&b) == -5);

  CORBA_fixed_add(&r, &a, &b);
  IS(r, "116.40");
  CORBA_fixed_sub(&r, &b, &a);
  IS(r, "-130.50");
  CORBA_fixed_sub(&r, &a, &a);
  IS(r, "0.00");
  CHECK(r._value[4] == 0x0c); /* zero is positive */

  /* Into a wider result: exact. */
  CORBA_fixed_mul(&w, &a, &b);
  IS(w, "-870.32250");
  /* Into the same scale: truncated toward zero. */
  CORBA_fixed_mul(&r, &a, &b);
  IS(r, "-870.32");
  CORBA_fixed_div(&w, &a, &b);
  IS(w, "-17.51063"); /* 123.45 / -7.05 = -17.510638... */
  CORBA_fixed_div(&r, &a, &b);
  IS(r, "-17.51");
  CORBA_fixed_set(&b, 0, 0);
  CORBA_fixed_div(&r, &a, &b);
  IS(r, "0.00");

  /* Mixed operands: the sum of 123.45 and 0.125. */
  CORBA_fixed_set(&f53, 0, 125);
  IS(f53, "0.125");
  CORBA_fixed_add(&w, &a, &f53);
  IS(w, "123.57500");
  CORBA_fixed_add(&r, &a, &f53);
  IS(r, "123.57");

  /* Digits above the result's dropped. */
  CORBA_fixed_set(&small, 12345, 0);
  IS(small, "2345");

  /* 31 digits. */
  CORBA_fixed_set(&big, 2147483647, 0);
  CORBA_fixed_mul(&big2, &big, &big);
  IS(big2, "4611686014132420609");
  CORBA_fixed_mul(&big, &big2, &big2); /* 38 digits: the 31 low ones */
  IS(big, "7892944572736998860269687930881");
  CHECK(CORBA_fixed_integer_part(&big) == 2147483647); /* clamped */

  any = CORBA_fixed_alloc(10);
  CHECK(any && any->_digits == 10 && any->_scale == 0);
  if (any) {
    any->_scale = 4;
    CORBA_fixed_div(any, &a, &f53);
    IS(*any, "987.6000");
    CORBA_free(any);
  }
  CHECK(CORBA_fixed_alloc(32) == NULL);

  if (failures) {
    fprintf(stderr, "%d failure(s)\n", failures);
    return 1;
  }
  return 0;
}
