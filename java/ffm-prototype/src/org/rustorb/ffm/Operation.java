// An operation name as the C ABI wants it, made once (a generated stub
// would keep one per operation in a static field).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb.ffm;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.nio.charset.StandardCharsets;

public final class Operation {
    final MemorySegment name;
    final long length;
    final boolean oneway;

    public Operation(String name, boolean oneway) {
        this.name = Arena.global().allocateFrom(name);
        this.length = name.getBytes(StandardCharsets.UTF_8).length;
        this.oneway = oneway;
    }
}
