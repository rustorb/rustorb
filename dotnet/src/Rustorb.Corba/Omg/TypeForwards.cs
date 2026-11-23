// Omg.Types moved to the assembly Rustorb.Types (the package of the same
// name, which Rustorb depends on): code built against Rustorb 0.7.0, which
// had them in Rustorb.Corba, finds them there.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System.Runtime.CompilerServices;

[assembly: TypeForwardedTo(typeof(Omg.Types.ISequence<>))]
[assembly: TypeForwardedTo(typeof(Omg.Types.Sequence<>))]
[assembly: TypeForwardedTo(typeof(Omg.Types.FactoryAttribute))]
[assembly: TypeForwardedTo(typeof(Omg.Types.OptionalAttribute))]
[assembly: TypeForwardedTo(typeof(Omg.Types.KeyAttribute))]
[assembly: TypeForwardedTo(typeof(Omg.Types.UnitAttribute))]
