/* The values of idl/AnyTestHidden.idl, which only the client knows (the
 * servers hold them as anys of types they have no code for).
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include "anytest_hidden.h"

#include <string.h>

#include "AnyTestHidden.h"

static CORBA_any* any_of(CORBA_TypeCode tc, const void* value) {
  CORBA_any a;
  memset(&a, 0, sizeof a);
  a._type = tc;
  a._value = (void*)value;
  return (CORBA_any*)rustorb_c_copy(&CORBA_any__desc, &a);
}

CORBA_any* anytest_hidden(const char* name) {
  if (strcmp(name, "secret") == 0) {
    AnyTest_Point p[1] = {{1, 2.0}};
    AnyTestHidden_Secret v = {(CORBA_char*)"s", {1, 1, p, CORBA_FALSE}, AnyTest_DARK};
    return any_of(TC_AnyTestHidden_Secret, &v);
  }
  if (strcmp(name, "pick") == 0) {
    AnyTestHidden_Pick v;
    memset(&v, 0, sizeof v);
    v._d = 'b';
    v._u.s.text = (CORBA_char*)"p";
    v._u.s.shade = AnyTest_LIGHT;
    return any_of(TC_AnyTestHidden_Pick, &v);
  }
  if (strcmp(name, "moods") == 0) {
    AnyTestHidden_Mood m[2] = {AnyTestHidden_SAD, AnyTestHidden_HAPPY};
    AnyTestHidden_Moods v = {2, 2, m, CORBA_FALSE};
    return any_of(TC_AnyTestHidden_Moods, &v);
  }
  return NULL;
}
