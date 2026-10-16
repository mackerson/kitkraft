ScriptName KitKraft:OracleControl extends ScriptObject
{KitKraft papyrus oracle fixture (ours): conditionals, nested while loops, logic, guards, structs.}

Struct Pair
    Int First = 1
    Float Second
EndStruct

Guard CountGuard ProtectsFunctionLogic

Int counter

Int Function Classify(Int n, Bool strict)
    If n < 0 && !strict
        Return -1
    ElseIf n == 0 || (strict && n < 10)
        Return 0
    ElseIf !(n > 100)
        Int steps = 0
        While n > 1
            If n % 2 == 0
                n /= 2
            Else
                n = 3 * n + 1
            EndIf
            steps += 1
            Int inner = 0
            While inner < 2 && steps < 50
                inner += 1
            EndWhile
        EndWhile
        Return steps
    Else
        Return 100
    EndIf
EndFunction

Pair Function MakePair(Int a)
    Pair p = new Pair
    p.First = a
    p.Second = a as Float
    LockGuard CountGuard
        counter += 1
    EndLockGuard
    Return p
EndFunction
