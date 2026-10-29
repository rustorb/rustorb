// The values of AnyTestHidden.idl, which only the clients use (see
// AnyTests).

package interop;

import AnyTest.Point;
import AnyTest.Shade;
import AnyTestHidden.Mood;
import AnyTestHidden.MoodsHelper;
import AnyTestHidden.Pick;
import AnyTestHidden.PickHelper;
import AnyTestHidden.Secret;
import AnyTestHidden.SecretHelper;
import org.omg.CORBA.Any;

final class Hidden {
    private Hidden() {}

    static Secret secret() {
        return new Secret("s", new Point[] {new Point(1, 2.0)}, Shade.DARK);
    }

    static Pick pick() {
        Pick p = new Pick();
        p.s(new Secret("p", new Point[0], Shade.LIGHT));
        return p;
    }

    static boolean make(Any a, String name) {
        switch (name) {
            case "secret": SecretHelper.insert(a, secret()); return true;
            case "pick": PickHelper.insert(a, pick()); return true;
            case "moods": MoodsHelper.insert(a, new Mood[] {Mood.SAD, Mood.HAPPY}); return true;
            default: return false;
        }
    }

    static boolean sameSecret(Secret a, Secret b) {
        if (!a.text.equals(b.text) || a.shade.value() != b.shade.value()
                || a.points.length != b.points.length) {
            return false;
        }
        for (int i = 0; i < a.points.length; ++i) {
            if (a.points[i].x != b.points[i].x || a.points[i].y != b.points[i].y) {
                return false;
            }
        }
        return true;
    }

    static boolean same(Any got, String name) {
        switch (name) {
            case "secret": return sameSecret(SecretHelper.extract(got), secret());
            case "pick": {
                Pick p = PickHelper.extract(got);
                return p.discriminator() == 'b' && sameSecret(p.s(), pick().s());
            }
            case "moods": {
                Mood[] m = MoodsHelper.extract(got);
                return m.length == 2 && m[0].value() == Mood._SAD && m[1].value() == Mood._HAPPY;
            }
            default: return false;
        }
    }
}
