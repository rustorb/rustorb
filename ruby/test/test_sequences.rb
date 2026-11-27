# frozen_string_literal: true

# Sequences of numbers, which the extension checks, packs and unpacks at
# once (Out#write_list, In#read_list), and Rustorb.octets_as_string: values
# through an IOP::Codec of the test ORB. (int8 sequences: test_idl4.rb.)

require_relative "helper"

class TestSequences < Minitest::Test
  def codec
    @codec ||= TestORB.orb.resolve_initial_references("CodecFactory")
                      .create_codec(IOP::Encoding.new(IOP::ENCODING_CDR_ENCAPS, 1, 2))
  end

  def seq(tc) = CORBA::TypeCode.create_sequence_tc(0, tc)

  def round(v, tc)
    t = seq(tc)
    codec.decode_value(codec.encode_value(CORBA::Any.to_any(v, t)), t).value
  end

  def test_numbers_round_trip
    big = 2**62 + 3
    {
      CORBA._tc_octet => [0, 1, 255],
      CORBA._tc_short => [-(2**15), 0, 2**15 - 1],
      CORBA._tc_ushort => [0, 2**16 - 1],
      CORBA._tc_long => [-(2**31), 7, 2**31 - 1],
      CORBA._tc_ulong => [0, 2**32 - 1],
      CORBA._tc_longlong => [-big, big, -(2**63), 2**63 - 1],
      CORBA._tc_ulonglong => [0, big, 2**64 - 1],
      CORBA._tc_double => [1.5, -2, 0.1],
      CORBA._tc_float => [1.5, -2, 3]
    }.each do |tc, v|
      floats = [CORBA::TK_FLOAT, CORBA::TK_DOUBLE].include?(tc.kind)
      assert_equal(floats ? v.map(&:to_f) : v, round(v, tc), tc.kind)
    end
    big = Array.new(100_000) { |i| i % 256 }
    assert_equal big, round(big, CORBA._tc_octet)
    assert_equal [1.5, 0.25], round([Rational(3, 2), 0.25], CORBA._tc_double)
  end

  def test_bad_elements
    [[CORBA._tc_octet, [1, 256]], [CORBA._tc_octet, [-1]], [CORBA._tc_short, [2**15]],
     [CORBA._tc_ulong, [1, 2, -1]], [CORBA._tc_ulonglong, [2**64]], [CORBA._tc_long, [1, "2"]],
     [CORBA._tc_double, [1.0, nil]]].each do |tc, v|
      e = assert_raises(CORBA::MARSHAL) { round(v, tc) }
      assert_equal CORBA::COMPLETED_NO, e.completed
    end
  end

  def test_octets_as_string
    data = (0..255).map(&:chr).join.b * 4
    assert_equal data.bytes, round(data, CORBA._tc_octet)
    Rustorb.octets_as_string = true
    r = round(data, CORBA._tc_octet)
    assert_equal data, r
    assert_equal Encoding::BINARY, r.encoding
    assert_equal [1, 2], round([1, 2], CORBA._tc_long)
  ensure
    Rustorb.octets_as_string = false
  end
end
