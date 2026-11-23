// The Omg.Types namespace of the IDL4 to C# mapping: the sequence interface
// (7.2.4.2.1) with rustorb's implementation, and the attributes the mapping
// uses. In a package of its own (Rustorb.Types) so that code generated for
// DDS (rustorb-dds's Rustorb.Dds) can use it without the ORB; Rustorb.Corba
// forwards these types here.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;

namespace Omg.Types
{
    /// <summary>An IDL sequence (IDL4 to C# 7.2.4.2.1): an IList&lt;T&gt; with the
    /// methods of List&lt;T&gt;.</summary>
    public interface ISequence<T> : IList<T>, IReadOnlyList<T>
    {
        new int Count { get; }
        new T this[int index] { get; set; }
        int Capacity { get; set; }
        void AddRange(IEnumerable<T> collection);
        ReadOnlyCollection<T> AsReadOnly();
        int BinarySearch(T item);
        int BinarySearch(T item, IComparer<T>? comparer);
        int BinarySearch(int index, int count, T item, IComparer<T>? comparer);
        List<TOutput> ConvertAll<TOutput>(Converter<T, TOutput> converter);
        void CopyTo(T[] array);
        void CopyTo(int index, T[] array, int arrayIndex, int count);
        bool Exists(Predicate<T> match);
        T? Find(Predicate<T> match);
        List<T> FindAll(Predicate<T> match);
        int FindIndex(Predicate<T> match);
        int FindIndex(int startIndex, Predicate<T> match);
        int FindIndex(int startIndex, int count, Predicate<T> match);
        T? FindLast(Predicate<T> match);
        int FindLastIndex(Predicate<T> match);
        int FindLastIndex(int startIndex, Predicate<T> match);
        int FindLastIndex(int startIndex, int count, Predicate<T> match);
        void ForEach(Action<T> action);
        List<T> GetRange(int index, int count);
        int IndexOf(T item, int index);
        int IndexOf(T item, int index, int count);
        void InsertRange(int index, IEnumerable<T> collection);
        int LastIndexOf(T item);
        int LastIndexOf(T item, int index);
        int LastIndexOf(T item, int index, int count);
        int RemoveAll(Predicate<T> match);
        void RemoveRange(int index, int count);
        void Reverse();
        void Reverse(int index, int count);
        void Sort();
        void Sort(Comparison<T> comparison);
        void Sort(IComparer<T>? comparer);
        void Sort(int index, int count, IComparer<T>? comparer);
        T[] ToArray();
        void TrimExcess();
        bool TrueForAll(Predicate<T> match);
    }

    /// <summary>rustorb's sequence: a List&lt;T&gt;. Equal when the elements are.</summary>
    public class Sequence<T> : List<T>, ISequence<T>, IEquatable<Sequence<T>>
    {
        public Sequence() { }
        public Sequence(int capacity) : base(capacity) { }
        public Sequence(IEnumerable<T> collection) : base(collection) { }

        public bool Equals(Sequence<T>? other) => Elements.Equal(this, other);
        public override bool Equals(object? obj) => obj is IList<T> l && Elements.Equal(this, l);
        public override int GetHashCode() => Elements.Hash(this);
    }

    /// <summary>Equality and hashes by the elements, nested sequences and
    /// arrays included (as Rustorb.Seq in Rustorb.Corba).</summary>
    internal static class Elements
    {
        internal static bool Equal<T>(IList<T>? a, IList<T>? b)
        {
            if (ReferenceEquals(a, b))
            {
                return true;
            }
            if (a == null || b == null || a.Count != b.Count)
            {
                return false;
            }
            var cmp = EqualityComparer<T>.Default;
            for (int i = 0; i < a.Count; i++)
            {
                if (!ElementEqual(a[i], b[i], cmp))
                {
                    return false;
                }
            }
            return true;
        }

        private static bool ElementEqual<TE>(TE x, TE y, IEqualityComparer<TE> cmp)
        {
            if (x is System.Collections.IList lx && y is System.Collections.IList ly && x is not string)
            {
                if (lx.Count != ly.Count)
                {
                    return false;
                }
                for (int i = 0; i < lx.Count; i++)
                {
                    if (!ElementEqual(lx[i], ly[i], EqualityComparer<object?>.Default))
                    {
                        return false;
                    }
                }
                return true;
            }
            return cmp.Equals(x, y);
        }

        internal static int Hash(System.Collections.IEnumerable e)
        {
            var h = new HashCode();
            foreach (object? o in e)
            {
                h.Add(o is System.Collections.IEnumerable inner and not string ? Hash(inner) : o?.GetHashCode() ?? 0);
            }
            return h.ToHashCode();
        }
    }

    /// <summary>Marks the factory operations of a value type (IDL4 to C# 7.6).</summary>
    [AttributeUsage(AttributeTargets.Method)]
    public class FactoryAttribute : Attribute
    {
        public FactoryAttribute() { IsFactory = true; }
        public bool IsFactory { get; set; }
    }

    /// <summary>@optional (IDL4 to C# 7.17).</summary>
    [AttributeUsage(AttributeTargets.All)]
    public class OptionalAttribute : Attribute
    {
        public OptionalAttribute() { IsOptional = true; }
        public bool IsOptional { get; set; }
    }

    /// <summary>@key (IDL4 to C# 7.17).</summary>
    [AttributeUsage(AttributeTargets.All)]
    public class KeyAttribute : Attribute
    {
        public KeyAttribute() { IsKey = true; }
        public bool IsKey { get; set; }
    }

    /// <summary>@unit (IDL4 to C# 7.17).</summary>
    [AttributeUsage(AttributeTargets.All)]
    public class UnitAttribute : Attribute
    {
        public UnitAttribute(string unitName) { UnitName = unitName; }
        public string UnitName { get; set; }
    }
}
