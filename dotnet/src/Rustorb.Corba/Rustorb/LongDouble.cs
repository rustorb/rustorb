// long double is decimal in the IDL4 to C# mapping (7.2.4.1). On the wire it
// is IEEE 754 binary128; these convert between the two, rounding to nearest
// (ties to even) where the other type has fewer digits.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Numerics;

namespace Rustorb
{
    internal static class LongDouble
    {
        private static readonly BigInteger Two112 = BigInteger.One << 112;
        private static readonly BigInteger Two113 = BigInteger.One << 113;
        private static readonly BigInteger Max96 = (BigInteger.One << 96) - 1;

        internal static void FromDecimal(decimal v, out ulong hi, out ulong lo)
        {
            int[] bits = decimal.GetBits(v);
            bool negative = (bits[3] & unchecked((int)0x80000000)) != 0;
            int scale = (bits[3] >> 16) & 0xFF;
            var m = new BigInteger((uint)bits[0]) | (new BigInteger((uint)bits[1]) << 32) | (new BigInteger((uint)bits[2]) << 64);
            ulong sign = negative ? 1UL << 63 : 0;
            if (m.IsZero)
            {
                hi = sign;
                lo = 0;
                return;
            }
            BigInteger den = BigInteger.Pow(10, scale);
            // q = round(m * 2^k / den) with 2^112 <= q < 2^113.
            int k = 112 - (int)(m.GetBitLength() - den.GetBitLength());
            BigInteger q, r;
            while (true)
            {
                BigInteger num = k >= 0 ? m << k : m;
                BigInteger d = k >= 0 ? den : den << -k;
                q = BigInteger.DivRem(num, d, out r);
                if (q >= Two113)
                {
                    k--;
                    continue;
                }
                if (q < Two112)
                {
                    k++;
                    continue;
                }
                BigInteger twice = r * 2;
                if (twice > d || (twice == d && !q.IsEven))
                {
                    q += 1;
                    if (q == Two113)
                    {
                        q >>= 1;
                        k--;
                    }
                }
                break;
            }
            int exponent = 112 - k + 16383;
            BigInteger mant = q - Two112;
            hi = sign | ((ulong)exponent << 48) | (ulong)(mant >> 64);
            lo = (ulong)(mant & ulong.MaxValue);
        }

        internal static decimal ToDecimal(ulong hi, ulong lo)
        {
            bool negative = (hi >> 63) != 0;
            int exponent = (int)((hi >> 48) & 0x7FFF);
            BigInteger mant = (new BigInteger(hi & 0xFFFF_FFFF_FFFFUL) << 64) | new BigInteger(lo);
            if (exponent == 0x7FFF)
            {
                throw new Corba.DATA_CONVERSION("an infinite or NaN long double has no decimal", 0,
                    Corba.CompletionStatus.COMPLETED_NO);
            }
            if (exponent == 0)
            {
                // Zero or subnormal: far below the smallest decimal.
                return negative ? -0m : 0m;
            }
            BigInteger m = Two112 + mant;
            int e = exponent - 16383 - 112;
            BigInteger num = e >= 0 ? m << e : m;
            BigInteger den = e >= 0 ? BigInteger.One : BigInteger.One << -e;
            BigInteger whole = BigInteger.Divide(num, den);
            if (whole > Max96)
            {
                throw new Corba.DATA_CONVERSION("a long double out of the range of decimal", 0,
                    Corba.CompletionStatus.COMPLETED_NO);
            }
            // The largest scale (at most 28) whose scaled value fits 96 bits.
            int scale = 0;
            BigInteger p10 = BigInteger.One;
            while (scale < 28 && BigInteger.Divide(num * p10 * 10, den) <= Max96)
            {
                scale++;
                p10 *= 10;
            }
            BigInteger q = BigInteger.DivRem(num * p10, den, out BigInteger r);
            BigInteger twice = r * 2;
            if (twice > den || (twice == den && !q.IsEven))
            {
                q += 1;
                if (q > Max96)
                {
                    q = BigInteger.Divide(q + 5, 10);
                    scale--;
                }
            }
            while (scale > 0 && (q % 10).IsZero)
            {
                q /= 10;
                scale--;
            }
            int l = (int)(uint)(q & uint.MaxValue);
            int mi = (int)(uint)((q >> 32) & uint.MaxValue);
            int h = (int)(uint)((q >> 64) & uint.MaxValue);
            return new decimal(l, mi, h, negative, (byte)scale);
        }
    }
}
