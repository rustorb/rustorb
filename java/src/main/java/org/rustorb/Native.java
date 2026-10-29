// The native methods of librustorb_jni (crates/rustorb-jni), one for each
// function of the C ABI the binding uses.
//
// Handles are longs (0 is NULL). A method that fails throws the CORBA
// exception of the C call; the classes of this package own the handles and
// release them.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

final class Native {
    private Native() {}

    static {
        Loader.load();
        int abi = init(Bridge.class);
        if (abi != 1) {
            throw new UnsatisfiedLinkError("librustorb_jni has ABI version " + abi + ", not 1");
        }
    }

    /** Forces the library to load (the static initializer). */
    static void ensureLoaded() {}

    private static native int init(Class<?> bridge);

    // --- ORB -------------------------------------------------------------
    static native long orbInit(String[] args);
    static native void orbRelease(long orb);
    static native long orbStringToObject(long orb, String s);
    static native String orbObjectToString(long orb, long obj);
    static native long orbResolveInitialReferences(long orb, String name)
            throws org.omg.CORBA.UserException;
    static native String[] orbListInitialServices(long orb);
    static native long orbRootPoa(long orb);
    static native void orbRun(long orb);
    static native boolean orbWorkPending(long orb);
    static native void orbPerformWork(long orb);
    static native void orbShutdown(long orb, boolean wait);
    static native void orbDestroy(long orb);
    static native long defaultRootPoa();
    /** Timeouts in microseconds, as the C ABI (rorb_orb_set_call_timeout) has them. */
    static native void orbSetCallTimeout(long orb, long us);
    static native long orbCallTimeout(long orb);
    static native void orbSetConnectTimeout(long orb, long us);
    static native long orbConnectTimeout(long orb);
    static native void setThreadCallTimeout(long us);
    static native long threadCallTimeout();

    // policies and registered initial references
    static native long orbCreatePolicy(long orb, int type, long any)
            throws org.omg.CORBA.PolicyError;
    static native void orbRegisterInitialReference(long orb, String name, long obj)
            throws org.omg.CORBA.ORBPackage.InvalidName;
    static native void orbRegisterLocalInitialReference(long orb, String name, Object obj)
            throws org.omg.CORBA.ORBPackage.InvalidName;
    static native Object orbResolveLocalInitialReference(long orb, String name);
    static native void orbSetPolicyOverrides(long orb, int[] types, long[] values, boolean add);
    static native long[] orbPolicyOverrides(long orb, int[] types);
    static native void setThreadPolicyOverrides(int[] types, long[] values, boolean add);
    static native long[] threadPolicyOverrides(int[] types);
    static native long objectSetPolicyOverrides(long obj, int[] types, long[] values,
            boolean add);
    static native long[] objectPolicyOverrides(long obj, int[] types);
    static native boolean objectClientPolicy(long obj, int type, long[] value);
    static native boolean objectValidateConnection(long obj);

    // --- object references -----------------------------------------------
    static native long objectClone(long obj);
    static native void objectRelease(long obj);
    static native long objectOrb(long obj);
    static native String objectTypeId(long obj);
    static native boolean objectIsA(long obj, String id);
    static native boolean objectNonExistent(long obj);
    static native boolean objectIsEquivalent(long a, long b);
    static native int objectHash(long obj, int maximum);
    static native void objectSetCallTimeout(long obj, long us);
    static native long objectCallTimeout(long obj);
    /** 0 (done), 2 (a user exception, taken by call) or 4 (call.error). */
    static native int objectInvoke(long obj, String op, boolean oneway, long timeoutUs, Call call);

    // --- output streams ----------------------------------------------------
    static native void outBoolean(long out, boolean v);
    static native void outOctet(long out, byte v);
    static native void outChar(long out, char v);
    static native void outWchar(long out, char v);
    static native void outShort(long out, short v);
    static native void outUshort(long out, short v);
    static native void outLong(long out, int v);
    static native void outUlong(long out, int v);
    static native void outLonglong(long out, long v);
    static native void outUlonglong(long out, long v);
    static native void outFloat(long out, float v);
    static native void outDouble(long out, double v);
    static native void outString(long out, String v);
    static native void outWstring(long out, String v);
    static native void outObject(long out, long obj);
    static native void outTypeCode(long out, long tc);
    static native void outAny(long out, long any);
    static native void outArrayByte(long out, int kind, byte[] v, int off, int len);
    static native void outArrayShort(long out, int kind, short[] v, int off, int len);
    static native void outArrayInt(long out, int kind, int[] v, int off, int len);
    static native void outArrayLong(long out, int kind, long[] v, int off, int len);
    static native void outArrayFloat(long out, int kind, float[] v, int off, int len);
    static native void outArrayDouble(long out, int kind, double[] v, int off, int len);
    static native boolean outFailed(long out);

    // --- input streams -----------------------------------------------------
    static native boolean inBoolean(long in);
    static native byte inOctet(long in);
    static native char inChar(long in);
    static native char inWchar(long in);
    static native short inShort(long in);
    static native short inUshort(long in);
    static native int inLong(long in);
    static native int inUlong(long in);
    static native long inLonglong(long in);
    static native long inUlonglong(long in);
    static native float inFloat(long in);
    static native double inDouble(long in);
    static native String inString(long in);
    static native String inWstring(long in);
    static native long inObject(long in);
    static native long inTypeCode(long in);
    static native long inAny(long in);
    static native void inArrayByte(long in, int kind, byte[] v, int off, int len);
    static native void inArrayShort(long in, int kind, short[] v, int off, int len);
    static native void inArrayInt(long in, int kind, int[] v, int off, int len);
    static native void inArrayLong(long in, int kind, long[] v, int off, int len);
    static native void inArrayFloat(long in, int kind, float[] v, int off, int len);
    static native void inArrayDouble(long in, int kind, double[] v, int off, int len);
    static native int inLength(long in, int minElementSize);
    static native void inFail(long in);
    static native boolean inFailed(long in);

    // valuetypes (RORB_VALUE_NULL, RORB_VALUE_INDIRECTION, RORB_VALUE_NEW)
    static final int VALUE_NULL = 0;
    static final int VALUE_INDIRECTION = 1;
    static final int VALUE_NEW = 2;
    static native void outNullValue(long out);
    static native boolean outBeginValue(long out, long key, String[] ids, boolean chunked);
    static native void outEndValue(long out);
    /** Fills slotCount with the slot and the number of repository ids. */
    static native int inValueHeader(long in, long[] slotCount);
    static native String inValueId(long in, int index);
    static native void inEndValue(long in);

    // Element kinds of the array methods (RORB_PRIM_*).
    static final int PRIM_OCTET = 0;
    static final int PRIM_SHORT = 1;
    static final int PRIM_USHORT = 2;
    static final int PRIM_LONG = 3;
    static final int PRIM_ULONG = 4;
    static final int PRIM_LONGLONG = 5;
    static final int PRIM_ULONGLONG = 6;
    static final int PRIM_FLOAT = 7;
    static final int PRIM_DOUBLE = 8;

    // --- servants ------------------------------------------------------------
    static native long servantPeerNew(Object servant);
    static native void servantPeerFree(long peer);
    static native long servantGet(long peer, String repositoryId);
    static native void servantRelease(long servant);
    static native long servantCurrentReference(long servant);
    static native long replyOutput(long reply);
    static native long replyUserException(long reply, String id);
    static native void replySystemException(long reply, String id, int minor, int completed);

    // --- POA -------------------------------------------------------------------
    static native long poaClone(long poa);
    static native void poaRelease(long poa);
    static native String poaName(long poa);
    static native long poaParent(long poa);
    static native long[] poaChildren(long poa);
    static native long poaTheManager(long poa);
    static native int[] poaPolicies(long poa);
    static native long poaCreatePoa(long poa, String name, long manager, int[] types, int[] values)
            throws org.omg.CORBA.UserException;
    static native long poaFindPoa(long poa, String name, boolean activateIt)
            throws org.omg.CORBA.UserException;
    static native void poaDestroy(long poa, boolean etherealize, boolean wait);
    static native byte[] poaActivateObject(long poa, long servant) throws org.omg.CORBA.UserException;
    static native void poaActivateObjectWithId(long poa, byte[] id, long servant)
            throws org.omg.CORBA.UserException;
    static native void poaDeactivateObject(long poa, byte[] id) throws org.omg.CORBA.UserException;
    static native byte[] poaServantToId(long poa, long servant) throws org.omg.CORBA.UserException;
    static native long poaServantToReference(long poa, long servant)
            throws org.omg.CORBA.UserException;
    static native Object poaIdToServant(long poa, byte[] id) throws org.omg.CORBA.UserException;
    static native long poaIdToReference(long poa, byte[] id) throws org.omg.CORBA.UserException;
    static native byte[] poaReferenceToId(long poa, long obj) throws org.omg.CORBA.UserException;
    static native Object poaReferenceToServant(long poa, long obj)
            throws org.omg.CORBA.UserException;
    static native long poaCreateReference(long poa, String intf) throws org.omg.CORBA.UserException;
    static native long poaCreateReferenceWithId(long poa, byte[] id, String intf)
            throws org.omg.CORBA.UserException;
    static native Object poaGetServant(long poa) throws org.omg.CORBA.UserException;
    static native void poaSetServant(long poa, long servant) throws org.omg.CORBA.UserException;
    /** set_servant_manager with a ServantLocator (locator) or a
     * ServantActivator; the callbacks go to Bridge. */
    static native void poaSetServantManager(long poa, Object manager, boolean locator)
            throws org.omg.CORBA.UserException;
    static native Object poaGetServantManager(long poa) throws org.omg.CORBA.UserException;
    /** null removes the adapter activator. */
    static native void poaSetTheActivator(long poa, Object activator);
    static native Object poaTheActivator(long poa);
    /** What incarnate or preinvoke raised (rorb_manager_error_*). */
    static native void managerErrorForward(long error, long obj);
    static native void managerErrorSystem(long error, String id, int minor, int completed);

    static native long currentGetPoa() throws org.omg.CORBA.UserException;
    static native byte[] currentGetObjectId() throws org.omg.CORBA.UserException;
    static native long currentGetReference() throws org.omg.CORBA.UserException;
    static native Object currentGetServant() throws org.omg.CORBA.UserException;

    static native long pmClone(long manager);
    static native void pmRelease(long manager);
    static native int pmGetState(long manager);
    static native void pmActivate(long manager) throws org.omg.CORBA.UserException;
    static native void pmHoldRequests(long manager, boolean wait) throws org.omg.CORBA.UserException;
    static native void pmDiscardRequests(long manager, boolean wait)
            throws org.omg.CORBA.UserException;
    static native void pmDeactivate(long manager, boolean etherealize, boolean wait)
            throws org.omg.CORBA.UserException;

    // --- TypeCodes ---------------------------------------------------------------
    static native long tcClone(long tc);
    static native void tcRelease(long tc);
    static native long tcSimple(int kind);
    static native long tcString(boolean wide, int bound);
    static native long tcFixed(short digits, short scale);
    static native long tcSequence(long content, int bound);
    static native long tcArray(long content, int length);
    static native long tcAlias(int kind, String id, String name, long content);
    static native long tcInterface(int kind, String id, String name);
    static native long tcStruct(int kind, String id, String name, String[] names, long[] types);
    static native long tcUnion(String id, String name, long discriminator, long[] labels,
            int defaultIndex, String[] names, long[] types);
    static native long tcEnum(String id, String name, String[] names);
    static native long tcRecursive(String id);
    static native long tcValue(String id, String name, short modifier, long base,
            String[] names, long[] types, short[] visibilities);
    static native void registerValueType(long tc);
    static native int tcKind(long tc);
    static native String tcId(long tc)
            throws org.omg.CORBA.TypeCodePackage.BadKind;
    static native String tcName(long tc)
            throws org.omg.CORBA.TypeCodePackage.BadKind;
    static native int tcMemberCount(long tc)
            throws org.omg.CORBA.TypeCodePackage.BadKind;
    static native String tcMemberName(long tc, int index)
            throws org.omg.CORBA.TypeCodePackage.BadKind, org.omg.CORBA.TypeCodePackage.Bounds;
    static native long tcMemberType(long tc, int index)
            throws org.omg.CORBA.TypeCodePackage.BadKind, org.omg.CORBA.TypeCodePackage.Bounds;
    static native long tcMemberLabel(long tc, int index)
            throws org.omg.CORBA.TypeCodePackage.BadKind, org.omg.CORBA.TypeCodePackage.Bounds;
    static native long tcDiscriminatorType(long tc)
            throws org.omg.CORBA.TypeCodePackage.BadKind;
    static native int tcDefaultIndex(long tc)
            throws org.omg.CORBA.TypeCodePackage.BadKind;
    static native int tcLength(long tc)
            throws org.omg.CORBA.TypeCodePackage.BadKind;
    static native long tcContentType(long tc)
            throws org.omg.CORBA.TypeCodePackage.BadKind;
    static native short tcFixedDigits(long tc)
            throws org.omg.CORBA.TypeCodePackage.BadKind;
    static native short tcFixedScale(long tc)
            throws org.omg.CORBA.TypeCodePackage.BadKind;
    static native short tcMemberVisibility(long tc, int index)
            throws org.omg.CORBA.TypeCodePackage.BadKind, org.omg.CORBA.TypeCodePackage.Bounds;
    static native short tcTypeModifier(long tc)
            throws org.omg.CORBA.TypeCodePackage.BadKind;
    static native long tcConcreteBaseType(long tc)
            throws org.omg.CORBA.TypeCodePackage.BadKind;
    static native boolean tcEqual(long a, long b);
    static native boolean tcEquivalent(long a, long b);
    static native long tcCompact(long tc);
    static native long tcUnaliased(long tc);

    // --- anys --------------------------------------------------------------------
    static native long anyNew();
    static native long anyClone(long any);
    static native void anyRelease(long any);
    static native long anyType(long any);
    static native void anySetType(long any, long tc);
    static native boolean anyEqual(long a, long b);
    static native boolean anyLabel(long any, long[] label);
    static native long anyInsert(long tc, Call call);
    static native void anyExtract(long any, Call call);

    // --- DynamicAny (crates/rustorb-jni/src/dynany.rs) ------------------------
    static native long dynCreate(long any, boolean truncate) throws org.omg.CORBA.UserException;
    static native long dynFromTypeCode(long tc) throws org.omg.CORBA.UserException;
    static native long dynClone(long d);
    static native void dynRelease(long d);
    static native int dynKind(long d);
    static native boolean dynSame(long a, long b);
    static native long dynType(long d) throws org.omg.CORBA.UserException;
    static native long dynTargetType(long d) throws org.omg.CORBA.UserException;
    static native long dynToAny(long d) throws org.omg.CORBA.UserException;
    static native long dynCopy(long d) throws org.omg.CORBA.UserException;
    static native long dynGetDynAny(long d) throws org.omg.CORBA.UserException;
    static native long dynCurrentComponent(long d) throws org.omg.CORBA.UserException;
    static native long dynGetDiscriminator(long d) throws org.omg.CORBA.UserException;
    static native long dynMember(long d) throws org.omg.CORBA.UserException;
    static native long dynGetBoxedValue(long d) throws org.omg.CORBA.UserException;
    static native long dynGetBoxedValueAsDynAny(long d) throws org.omg.CORBA.UserException;
    static native int dynComponentCount(long d) throws org.omg.CORBA.UserException;
    static native int dynGetAsUlong(long d) throws org.omg.CORBA.UserException;
    static native int dynCurrentMemberKind(long d) throws org.omg.CORBA.UserException;
    static native int dynMemberCount(long d) throws org.omg.CORBA.UserException;
    static native int dynDiscriminatorKind(long d) throws org.omg.CORBA.UserException;
    static native int dynMemberKind(long d) throws org.omg.CORBA.UserException;
    static native int dynGetLength(long d) throws org.omg.CORBA.UserException;
    static native int dynElementCount(long d) throws org.omg.CORBA.UserException;
    static native boolean dynNext(long d) throws org.omg.CORBA.UserException;
    static native boolean dynHasNoActiveMember(long d) throws org.omg.CORBA.UserException;
    static native boolean dynIsNull(long d) throws org.omg.CORBA.UserException;
    static native String dynGetFixedValue(long d) throws org.omg.CORBA.UserException;
    static native String dynGetAsString(long d) throws org.omg.CORBA.UserException;
    static native String dynCurrentMemberName(long d) throws org.omg.CORBA.UserException;
    static native String dynMemberName(long d) throws org.omg.CORBA.UserException;
    static native void dynDestroy(long d) throws org.omg.CORBA.UserException;
    static native void dynRewind(long d) throws org.omg.CORBA.UserException;
    static native void dynNoImplement(long d) throws org.omg.CORBA.UserException;
    static native void dynSetToDefaultMember(long d) throws org.omg.CORBA.UserException;
    static native void dynSetToNoActiveMember(long d) throws org.omg.CORBA.UserException;
    static native void dynSetToNull(long d) throws org.omg.CORBA.UserException;
    static native void dynSetToValue(long d) throws org.omg.CORBA.UserException;
    static native void dynAssign(long d, long h) throws org.omg.CORBA.UserException;
    static native void dynFromAny(long d, long h) throws org.omg.CORBA.UserException;
    static native void dynInsertDynAny(long d, long h) throws org.omg.CORBA.UserException;
    static native void dynSetDiscriminator(long d, long h) throws org.omg.CORBA.UserException;
    static native void dynSetBoxedValue(long d, long h) throws org.omg.CORBA.UserException;
    static native void dynSetBoxedValueAsDynAny(long d, long h) throws org.omg.CORBA.UserException;
    static native void dynInsert(long d, int kind, long any) throws org.omg.CORBA.UserException;
    static native void dynInsertSeq(long d, int kind, long any) throws org.omg.CORBA.UserException;
    static native long dynGet(long d, int kind) throws org.omg.CORBA.UserException;
    static native long dynGetSeq(long d, int kind) throws org.omg.CORBA.UserException;
    static native void dynSetAsUlong(long d, int value) throws org.omg.CORBA.UserException;
    static native void dynSetLength(long d, int length) throws org.omg.CORBA.UserException;
    static native boolean dynEqual(long d, long other) throws org.omg.CORBA.UserException;
    static native boolean dynSeek(long d, int index) throws org.omg.CORBA.UserException;
    static native boolean dynSetFixedValue(long d, String value) throws org.omg.CORBA.UserException;
    static native void dynSetAsString(long d, String value) throws org.omg.CORBA.UserException;
    /** The member's DynAny; its name goes in name[0]. */
    static native long dynMemberAt(long d, int index, String[] name) throws org.omg.CORBA.UserException;
    /** set_members (values are anys) or set_members_as_dyn_any (DynAnys). */
    static native void dynSetMembers(long d, String[] names, long[] values, boolean asDynAny)
            throws org.omg.CORBA.UserException;
    static native long dynElementAt(long d, int index) throws org.omg.CORBA.UserException;
    /** set_elements (anys) or set_elements_as_dyn_any (DynAnys). */
    static native void dynSetElements(long d, long[] values, boolean asDynAny) throws org.omg.CORBA.UserException;
}
