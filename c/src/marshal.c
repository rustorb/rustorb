/* The generic reader and writer of values described by descriptors. */

#include "internal.h"

static int fail(rustorb_c_err* err, const char* id, CORBA_unsigned_long minor) {
  err->id = id;
  err->minor = minor;
  return -1;
}

static const rustorb_c_type* unalias(const rustorb_c_type* t) {
  while (t && t->kind == CORBA_tk_alias) t = t->content;
  return t;
}

/* An enum value by the size of the C enum. */
static CORBA_unsigned_long get_enum(const rustorb_c_type* t, const void* p) {
  switch (t->size) {
    case 1: return *(const uint8_t*)p;
    case 2: return *(const uint16_t*)p;
    case 8: return (CORBA_unsigned_long) * (const uint64_t*)p;
    default: return *(const uint32_t*)p;
  }
}

static void set_enum(const rustorb_c_type* t, void* p, CORBA_unsigned_long v) {
  switch (t->size) {
    case 1: *(uint8_t*)p = (uint8_t)v; break;
    case 2: *(uint16_t*)p = (uint16_t)v; break;
    case 8: *(uint64_t*)p = v; break;
    default: *(uint32_t*)p = v; break;
  }
}

uint64_t rustorb_c_label(const rustorb_c_type* disc, const void* p) {
  const rustorb_c_type* t = unalias(disc);
  switch (t->kind) {
    case CORBA_tk_short: return (uint64_t)(int64_t) * (const CORBA_short*)p;
    case CORBA_tk_long: return (uint64_t)(int64_t) * (const CORBA_long*)p;
    case CORBA_tk_longlong: return (uint64_t) * (const CORBA_long_long*)p;
    case CORBA_tk_ushort: return *(const CORBA_unsigned_short*)p;
    case CORBA_tk_ulong: return *(const CORBA_unsigned_long*)p;
    case CORBA_tk_ulonglong: return *(const CORBA_unsigned_long_long*)p;
    case CORBA_tk_boolean: return *(const CORBA_boolean*)p ? 1 : 0;
    case CORBA_tk_char: return *(const unsigned char*)p;
    case CORBA_tk_wchar: return *(const CORBA_wchar*)p;
    case CORBA_tk_enum: return get_enum(t, p);
    default: return 0;
  }
}

/* The kind of rorb_out_write_array for elements of t, or -1. */
static int prim(const rustorb_c_type* t) {
  switch (t->kind) {
    case CORBA_tk_char:
    case CORBA_tk_octet: return RORB_PRIM_OCTET;
    case CORBA_tk_short: return RORB_PRIM_SHORT;
    case CORBA_tk_ushort: return RORB_PRIM_USHORT;
    case CORBA_tk_long: return RORB_PRIM_LONG;
    case CORBA_tk_ulong: return RORB_PRIM_ULONG;
    case CORBA_tk_longlong: return RORB_PRIM_LONGLONG;
    case CORBA_tk_ulonglong: return RORB_PRIM_ULONGLONG;
    case CORBA_tk_float: return RORB_PRIM_FLOAT;
    case CORBA_tk_double: return RORB_PRIM_DOUBLE;
    default: return -1;
  }
}

/* The fewest bytes an element takes on the wire. */
static size_t min_size(const rustorb_c_type* t) {
  t = unalias(t);
  switch (t->kind) {
    case CORBA_tk_short:
    case CORBA_tk_ushort:
    case CORBA_tk_wchar: return 2;
    case CORBA_tk_long:
    case CORBA_tk_ulong:
    case CORBA_tk_float:
    case CORBA_tk_enum:
    case CORBA_tk_string:
    case CORBA_tk_wstring:
    case CORBA_tk_sequence: return 4;
    case CORBA_tk_longlong:
    case CORBA_tk_ulonglong:
    case CORBA_tk_double: return 8;
    case CORBA_tk_longdouble: return 16;
    default: return 1;
  }
}

static int write_fixed(rorb_out* out, const rustorb_c_type* t, const void* p,
                       rustorb_c_err* err) {
  const CORBA_octet* v = (const CORBA_octet*)p + 2 * sizeof(CORBA_short);
  size_t bytes = (size_t)(t->digits + 2) / 2;
  uint8_t digits[32];
  size_t i, nibbles = bytes * 2 - 1; /* without the sign */
  int negative = (v[bytes - 1] & 0x0f) == 0x0d;
  if (t->digits > 31) return fail(err, ex_CORBA_BAD_PARAM, 0);
  for (i = 0; i < t->digits; i++) {
    size_t n = nibbles - t->digits + i;
    uint8_t d = (uint8_t)(n % 2 == 0 ? v[n / 2] >> 4 : v[n / 2] & 0x0f);
    if (d > 9) return fail(err, ex_CORBA_DATA_CONVERSION, 0);
    digits[i] = d;
  }
  rorb_out_write_fixed(out, digits, t->digits, negative);
  return 0;
}

static void read_fixed(rorb_in* in, const rustorb_c_type* t, void* p) {
  CORBA_octet* v = (CORBA_octet*)p + 2 * sizeof(CORBA_short);
  size_t bytes = (size_t)(t->digits + 2) / 2;
  uint8_t digits[32];
  size_t i, nibbles = bytes * 2 - 1;
  int negative;
  if (t->digits > 31) {
    rorb_in_fail(in);
    return;
  }
  negative = rorb_in_read_fixed(in, digits, t->digits);
  ((CORBA_unsigned_short*)p)[0] = t->digits;
  ((CORBA_short*)p)[1] = t->scale;
  memset(v, 0, bytes);
  for (i = 0; i < t->digits; i++) {
    size_t n = nibbles - t->digits + i;
    if (n % 2 == 0)
      v[n / 2] |= (CORBA_octet)(digits[i] << 4);
    else
      v[n / 2] |= digits[i];
  }
  v[bytes - 1] |= negative ? 0x0d : 0x0c;
}

int rustorb_c_write_value(rorb_out* out, const rustorb_c_type* t, const void* p,
                          rustorb_c_err* err) {
  /* An exception's value in an any starts with its repository id. */
  if (t->kind == CORBA_tk_except) rorb_out_write_string(out, t->id, strlen(t->id));
  return rustorb_c_write(out, t, p, err);
}

void rustorb_c_read_value(rorb_in* in, const rustorb_c_type* t, void* p) {
  if (t->kind == CORBA_tk_except) {
    size_t len;
    if (!rorb_in_read_string(in, &len)) return;
  }
  rustorb_c_read(in, t, p);
}

static int write_any(rorb_out* out, const CORBA_any* a, rustorb_c_err* err) {
  rorb_typecode* tc = rustorb_c_tc_handle(a->_type);
  const rustorb_c_type* t;
  uint32_t kind;
  if (!a->_type) {
    /* An empty any: tk_null. */
    rorb_out_write_typecode(out, rustorb_c_tc_handle(TC_null));
    return 0;
  }
  if (!tc) return fail(err, ex_CORBA_BAD_PARAM, 0);
  kind = rorb_tc_kind(tc);
  if (!a->_value) {
    if (a->_rustorb) {
      rorb_out_write_any(out, (rorb_any*)a->_rustorb);
      return 0;
    }
    if (kind == CORBA_tk_null || kind == CORBA_tk_void) {
      rorb_out_write_typecode(out, tc);
      return 0;
    }
    return fail(err, ex_CORBA_BAD_PARAM, 0);
  }
  t = a->_type->desc ? a->_type->desc : rustorb_c_desc_for_tc(tc);
  if (!t) return fail(err, ex_CORBA_BAD_TYPECODE, 0);
  rorb_out_write_typecode(out, tc);
  return rustorb_c_write_value(out, t, a->_value, err);
}

static void read_any(rorb_in* in, CORBA_any* a) {
  rorb_typecode* tc = rorb_in_read_typecode(in);
  const rustorb_c_type* t;
  uint32_t kind;
  if (!tc) return;
  kind = rorb_tc_kind(tc);
  t = (kind == CORBA_tk_null || kind == CORBA_tk_void) ? NULL : rustorb_c_desc_for_tc(tc);
  a->_type = rustorb_c_wrap_tc(tc, t);
  a->_release = CORBA_TRUE;
  if (kind == CORBA_tk_null || kind == CORBA_tk_void) return;
  if (t) {
    a->_value = rustorb_c_alloc(t, 1);
    if (!a->_value) {
      rorb_in_fail(in);
      return;
    }
    rustorb_c_read_value(in, t, a->_value);
  } else {
    /* A type the program has no descriptor for: kept as it came. */
    a->_rustorb = rorb_in_read_any_value(in, tc);
  }
}

int rustorb_c_write(rorb_out* out, const rustorb_c_type* t, const void* p, rustorb_c_err* err) {
  CORBA_unsigned_long i;
  switch (t->kind) {
    case CORBA_tk_null:
    case CORBA_tk_void: return 0;
    case CORBA_tk_short: rorb_out_write_short(out, *(const CORBA_short*)p); return 0;
    case CORBA_tk_long: rorb_out_write_long(out, *(const CORBA_long*)p); return 0;
    case CORBA_tk_longlong: rorb_out_write_longlong(out, *(const CORBA_long_long*)p); return 0;
    case CORBA_tk_ushort: rorb_out_write_ushort(out, *(const CORBA_unsigned_short*)p); return 0;
    case CORBA_tk_ulong: rorb_out_write_ulong(out, *(const CORBA_unsigned_long*)p); return 0;
    case CORBA_tk_ulonglong:
      rorb_out_write_ulonglong(out, *(const CORBA_unsigned_long_long*)p);
      return 0;
    case CORBA_tk_float: rorb_out_write_float(out, *(const CORBA_float*)p); return 0;
    case CORBA_tk_double: rorb_out_write_double(out, *(const CORBA_double*)p); return 0;
    case CORBA_tk_longdouble: {
      uint64_t hi, lo;
      rorb_longdouble_from_double((double)*(const CORBA_long_double*)p, &hi, &lo);
      rorb_out_write_longdouble(out, hi, lo);
      return 0;
    }
    case CORBA_tk_boolean: rorb_out_write_boolean(out, *(const CORBA_boolean*)p != 0); return 0;
    case CORBA_tk_char: rorb_out_write_char(out, *(const CORBA_char*)p); return 0;
    case CORBA_tk_wchar: rorb_out_write_wchar(out, *(const CORBA_wchar*)p); return 0;
    case CORBA_tk_octet: rorb_out_write_octet(out, *(const CORBA_octet*)p); return 0;
    case CORBA_tk_enum: {
      CORBA_unsigned_long v = get_enum(t, p);
      if (v >= t->count) return fail(err, ex_CORBA_BAD_PARAM, 0);
      rorb_out_write_ulong(out, v);
      return 0;
    }
    case CORBA_tk_string: {
      const char* s = *(const char* const*)p;
      size_t len;
      if (!s) return fail(err, ex_CORBA_BAD_PARAM, 0);
      len = strlen(s);
      if (t->length && len > t->length) return fail(err, ex_CORBA_BAD_PARAM, 0);
      rorb_out_write_string(out, s, len);
      return 0;
    }
    case CORBA_tk_wstring: {
      const CORBA_wchar* s = *(const CORBA_wchar* const*)p;
      CORBA_unsigned_long len;
      if (!s) return fail(err, ex_CORBA_BAD_PARAM, 0);
      len = CORBA_wstring_len(s);
      if (t->length && len > t->length) return fail(err, ex_CORBA_BAD_PARAM, 0);
      rorb_out_write_wstring(out, s, len);
      return 0;
    }
    case CORBA_tk_objref:
    case CORBA_tk_abstract_interface:
    case CORBA_tk_local_interface: {
      CORBA_Object o = *(const CORBA_Object*)p;
      if (!o) {
        rorb_out_write_object(out, NULL);
        return 0;
      }
      /* A local object (a POA, say) cannot be sent: MARSHAL minor 4. */
      if (o->kind != OBJ_REF) return fail(err, ex_CORBA_MARSHAL, 4);
      rorb_out_write_object(out, (rorb_object*)o->handle);
      return 0;
    }
    case CORBA_tk_TypeCode: {
      rorb_typecode* tc = rustorb_c_tc_handle(*(const CORBA_TypeCode*)p);
      if (!tc) return fail(err, ex_CORBA_BAD_PARAM, 0);
      rorb_out_write_typecode(out, tc);
      return 0;
    }
    case CORBA_tk_any: return write_any(out, (const CORBA_any*)p, err);
    case CORBA_tk_struct:
    case CORBA_tk_except:
      for (i = 0; i < t->count; i++)
        if (rustorb_c_write(out, t->members[i].type, (const char*)p + t->members[i].offset, err))
          return -1;
      return 0;
    case CORBA_tk_value: {
      /* A valuetype with the state of the members (only
         Messaging::ExceptionHolder: the C mapping has no valuetypes). */
      size_t len = strlen(t->id);
      if (rorb_out_begin_value(out, (uint64_t)(uintptr_t)p, 1, &t->id, &len, 0)) {
        for (i = 0; i < t->count; i++)
          if (rustorb_c_write(out, t->members[i].type, (const char*)p + t->members[i].offset,
                              err))
            return -1;
        rorb_out_end_value(out);
      }
      return 0;
    }
    case CORBA_tk_union: {
      int64_t e;
      if (rustorb_c_write(out, t->content, p, err)) return -1;
      if (rustorb_c_union_entry(t, p, &e))
        return rustorb_c_write(out, t->members[e].type, (const char*)p + t->members[e].offset,
                               err);
      return 0;
    }
    case CORBA_tk_sequence: {
      const CORBA_sequence_octet* s = (const CORBA_sequence_octet*)p;
      const rustorb_c_type* c = unalias(t->content);
      int k = prim(c);
      if (t->length && s->_length > t->length) return fail(err, ex_CORBA_BAD_PARAM, 0);
      if (s->_length && !s->_buffer) return fail(err, ex_CORBA_BAD_PARAM, 0);
      rorb_out_write_ulong(out, s->_length);
      if (k >= 0) {
        rorb_out_write_array(out, k, s->_buffer, s->_length);
        return 0;
      }
      for (i = 0; i < s->_length; i++)
        if (rustorb_c_write(out, t->content, (const char*)s->_buffer + i * t->content->size, err))
          return -1;
      return 0;
    }
    case CORBA_tk_array: {
      const rustorb_c_type* c = unalias(t->content);
      int k = prim(c);
      if (k >= 0) {
        rorb_out_write_array(out, k, p, t->length);
        return 0;
      }
      for (i = 0; i < t->length; i++)
        if (rustorb_c_write(out, t->content, (const char*)p + i * t->content->size, err))
          return -1;
      return 0;
    }
    case CORBA_tk_alias: return rustorb_c_write(out, t->content, p, err);
    case CORBA_tk_fixed: return write_fixed(out, t, p, err);
    default: return fail(err, ex_CORBA_BAD_TYPECODE, 0);
  }
}

static CORBA_char* read_string(rorb_in* in, const rustorb_c_type* t) {
  size_t len;
  const char* s = rorb_in_read_string(in, &len);
  if (rorb_in_failed(in)) return NULL;
  if (t->length && len > t->length) {
    rorb_in_fail(in);
    return NULL;
  }
  return rustorb_c_strndup(s, len);
}

static CORBA_wchar* read_wstring(rorb_in* in, const rustorb_c_type* t) {
  size_t len;
  const uint16_t* s = rorb_in_read_wstring(in, &len);
  CORBA_wchar* r;
  if (rorb_in_failed(in)) return NULL;
  if (t->length && len > t->length) {
    rorb_in_fail(in);
    return NULL;
  }
  r = CORBA_wstring_alloc((CORBA_unsigned_long)len);
  if (r && len) memcpy(r, s, len * sizeof(CORBA_wchar));
  return r;
}

void rustorb_c_read(rorb_in* in, const rustorb_c_type* t, void* p) {
  CORBA_unsigned_long i;
  if (rorb_in_failed(in)) return;
  switch (t->kind) {
    case CORBA_tk_null:
    case CORBA_tk_void: return;
    case CORBA_tk_short: *(CORBA_short*)p = rorb_in_read_short(in); return;
    case CORBA_tk_long: *(CORBA_long*)p = rorb_in_read_long(in); return;
    case CORBA_tk_longlong: *(CORBA_long_long*)p = rorb_in_read_longlong(in); return;
    case CORBA_tk_ushort: *(CORBA_unsigned_short*)p = rorb_in_read_ushort(in); return;
    case CORBA_tk_ulong: *(CORBA_unsigned_long*)p = rorb_in_read_ulong(in); return;
    case CORBA_tk_ulonglong: *(CORBA_unsigned_long_long*)p = rorb_in_read_ulonglong(in); return;
    case CORBA_tk_float: *(CORBA_float*)p = rorb_in_read_float(in); return;
    case CORBA_tk_double: *(CORBA_double*)p = rorb_in_read_double(in); return;
    case CORBA_tk_longdouble: {
      uint64_t hi = 0, lo = 0;
      rorb_in_read_longdouble(in, &hi, &lo);
      *(CORBA_long_double*)p = (CORBA_long_double)rorb_longdouble_to_double(hi, lo);
      return;
    }
    case CORBA_tk_boolean: *(CORBA_boolean*)p = rorb_in_read_boolean(in) ? 1 : 0; return;
    case CORBA_tk_char: *(CORBA_char*)p = rorb_in_read_char(in); return;
    case CORBA_tk_wchar: *(CORBA_wchar*)p = rorb_in_read_wchar(in); return;
    case CORBA_tk_octet: *(CORBA_octet*)p = rorb_in_read_octet(in); return;
    case CORBA_tk_enum: {
      CORBA_unsigned_long v = rorb_in_read_ulong(in);
      if (v >= t->count) {
        rorb_in_fail(in);
        return;
      }
      set_enum(t, p, v);
      return;
    }
    case CORBA_tk_string: *(CORBA_char**)p = read_string(in, t); return;
    case CORBA_tk_wstring: *(CORBA_wchar**)p = read_wstring(in, t); return;
    case CORBA_tk_objref:
    case CORBA_tk_abstract_interface:
    case CORBA_tk_local_interface:
      *(CORBA_Object*)p = rustorb_c_wrap(OBJ_REF, rorb_in_read_object(in));
      return;
    case CORBA_tk_TypeCode: {
      rorb_typecode* tc = rorb_in_read_typecode(in);
      *(CORBA_TypeCode*)p = tc ? rustorb_c_wrap_tc(tc, NULL) : NULL;
      return;
    }
    case CORBA_tk_any: read_any(in, (CORBA_any*)p); return;
    case CORBA_tk_struct:
    case CORBA_tk_except:
      for (i = 0; i < t->count; i++)
        rustorb_c_read(in, t->members[i].type, (char*)p + t->members[i].offset);
      return;
    case CORBA_tk_value: {
      size_t slot = 0, ids = 0;
      int32_t r = rorb_in_read_value_header(in, &slot, &ids);
      if (r == RORB_VALUE_INDIRECTION) {
        const void* v = rorb_in_get_value(in, slot);
        if (v)
          rustorb_c_copy_value(t, p, v);
        else
          rorb_in_fail(in);
      } else if (r == RORB_VALUE_NEW) {
        rorb_in_set_value(in, slot, p, NULL);
        for (i = 0; i < t->count; i++)
          rustorb_c_read(in, t->members[i].type, (char*)p + t->members[i].offset);
        rorb_in_end_value(in);
      }
      /* A null value stays zeroed. */
      return;
    }
    case CORBA_tk_union: {
      int64_t e;
      rustorb_c_read(in, t->content, p);
      if (!rorb_in_failed(in) && rustorb_c_union_entry(t, p, &e))
        rustorb_c_read(in, t->members[e].type, (char*)p + t->members[e].offset);
      return;
    }
    case CORBA_tk_sequence: {
      CORBA_sequence_octet* s = (CORBA_sequence_octet*)p;
      const rustorb_c_type* c = unalias(t->content);
      int k = prim(c);
      uint32_t len = rorb_in_read_length(in, min_size(c));
      if (rorb_in_failed(in)) return;
      if (t->length && len > t->length) {
        rorb_in_fail(in);
        return;
      }
      s->_release = CORBA_TRUE;
      if (!len) return;
      if (k >= 0) {
        /* Filled by the read (zeroed below if it fails). */
        s->_buffer = (CORBA_octet*)rustorb_c_alloc_filled(t->content, len);
        if (!s->_buffer) {
          rorb_in_fail(in);
          return;
        }
        s->_maximum = s->_length = len;
        rorb_in_read_array(in, k, s->_buffer, len);
        if (rorb_in_failed(in)) memset(s->_buffer, 0, (size_t)len * t->content->size);
        return;
      }
      s->_buffer = (CORBA_octet*)rustorb_c_alloc(t->content, len);
      if (!s->_buffer) {
        rorb_in_fail(in);
        return;
      }
      s->_maximum = s->_length = len;
      for (i = 0; i < len && !rorb_in_failed(in); i++)
        rustorb_c_read(in, t->content, (char*)s->_buffer + i * t->content->size);
      return;
    }
    case CORBA_tk_array: {
      const rustorb_c_type* c = unalias(t->content);
      int k = prim(c);
      if (k >= 0) {
        rorb_in_read_array(in, k, p, t->length);
        return;
      }
      for (i = 0; i < t->length && !rorb_in_failed(in); i++)
        rustorb_c_read(in, t->content, (char*)p + i * t->content->size);
      return;
    }
    case CORBA_tk_alias: rustorb_c_read(in, t->content, p); return;
    case CORBA_tk_fixed: read_fixed(in, t, p); return;
    default: rorb_in_fail(in); return;
  }
}
