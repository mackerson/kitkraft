ScriptName KitKraft:OracleFloats extends ScriptObject
{KitKraft papyrus oracle fixture (ours): float and int arithmetic, conversions, comparisons.}

Float Function Mix(Float a, Int b)
    Float x = a * 2.5 - b / 4.0
    x += 0.125
    x -= -1.0
    x *= 3
    x /= 2.0
    Int i = x as Int
    Int m = i % 3
    Float neg = -x
    If x > 1000.0 || neg <= -0.5
        Return 1.0 / 3.0
    EndIf
    Return (i + m) as Float + neg
EndFunction
