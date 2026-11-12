// Part of the IDL to Java mapping (org.omg.Messaging), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
package org.omg.Messaging;

/**
 * Messaging::SYNC_WITH_SERVER: a oneway call returns once the server has the
 * request and its target, before the servant runs.
 */
public interface SYNC_WITH_SERVER {
    short value = 2;
}
