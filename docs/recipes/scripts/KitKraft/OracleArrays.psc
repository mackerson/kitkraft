ScriptName KitKraft:OracleArrays extends ScriptObject
{KitKraft papyrus oracle fixture (ours): array creation, indexing, length, find, add, remove.}

Int[] Property Values Auto

Int Function Sum()
    If Values == None
        Values = new Int[4]
    EndIf
    Values[0] = 5
    Values.Add(9)
    Values.Add(3, 2)
    Int total = 0
    Int i = 0
    While i < Values.Length
        total += Values[i]
        i += 1
    EndWhile
    Int at = Values.Find(9)
    Values.Remove(0)
    String[] names = new String[2]
    names[1] = "two"
    Int found = names.RFind("two")
    Values.Clear()
    Return total + at + found
EndFunction
