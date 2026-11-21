// The test classes run one after the other: each fixture's ORB.init gives
// the process's ORB, which the fixture destroys at its end.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

[assembly: Xunit.CollectionBehavior(DisableTestParallelization = true)]
