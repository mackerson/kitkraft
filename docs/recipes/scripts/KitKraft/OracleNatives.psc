ScriptName KitKraft:OracleNatives extends ObjectReference
{KitKraft papyrus oracle fixture (ours): global and member native calls, casts, defaults.}

Function Run()
    Actor player = Game.GetPlayer()
    Float d = GetDistance(player)
    Int r = Utility.RandomInt()
    Int r2 = Utility.RandomInt(5, 9)
    Float a = Math.abs(-3.25)
    Int c = Math.Ceiling(d)
    Utility.Wait(0.5)
    Debug.Trace("distance " + d, 1)
    ObjectReference asRef = player as ObjectReference
    Bool ok = Activate(asRef)
    Debug.Trace("ok " + ok + " " + r + " " + r2 + " " + a + " " + c)
    Twice(c)
EndFunction

Int Function Twice(Int value) Global
    Return value * 2
EndFunction
