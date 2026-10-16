ScriptName KitKraft:OracleProperties extends ObjectReference
{KitKraft papyrus oracle fixture (ours): property kinds, flags, defaults and groups.}

Group Settings
    Int Property Count = 3 Auto
    {An auto property with a default.}
    Float Property Scale = 1.5 Auto Const
    Bool Property Enabled = True Auto Conditional
    String Property Label = "kit" Auto Const Mandatory
EndGroup

Group References CollapsedOnRef
    ObjectReference Property Target Auto
    Keyword Property Marker Auto Const
EndGroup

Int Property Hidden1 = 7 Auto Hidden

Int myLimit = 10
Int Property Limit Hidden
    Int Function Get()
        Return myLimit
    EndFunction
    Function Set(Int value)
        If value > 0
            myLimit = value
        EndIf
    EndFunction
EndProperty

Float Property Half
    Float Function Get()
        Return Scale / 2.0
    EndFunction
EndProperty
