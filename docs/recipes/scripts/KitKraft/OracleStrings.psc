ScriptName KitKraft:OracleStrings extends ScriptObject
{KitKraft papyrus oracle fixture (ours): string literals, escapes, concatenation, casts.}

String Function Describe(Int count, Float ratio, Bool flag)
    String s = "count=" + count
    s += ", ratio=" + ratio
    s = s + ", flag=" + flag
    s += "\n\ttab \"quoted\" back\\slash"
    If s == ""
        Return "empty"
    ElseIf s != "count=0"
        Return s + "!"
    EndIf
    String n = count as String
    Int back = n as Int
    Return n + back
EndFunction
