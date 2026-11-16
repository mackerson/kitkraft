ScriptName KitKraft:VerifyQuestReport extends Quest
{KitKraft bench verification quest (docs/bench/verify-mod.md, "Self-reporting quest"):
attached to the start-game-enabled quest KKQ01Qust of KitKraftVerifyQuest.esm. When the
quest starts (on the first load with the plugin enabled) it writes Debug.Trace lines that
start with "KKQ-" and their probe row: that the quest script runs, that its Marker property
holds the reference the same build placed, and that stage 10, whose fragment script
(Fragments:Quests:QF_KKQ01Qust) writes its own line, is done after SetStage.}

ObjectReference Property Marker Auto Const
{The placed reference KKQ70Marker, named in the recipe by its [[place]] (new:marker).}
Form Property MarkerBase Auto Const
{The marker's base (Starfield.esm Crate_Seafood03).}

Event OnQuestInit()
    Debug.Trace("KKQ-Q01 quest script running")
    Bool hasMarker = Marker != None
    Bool baseIs = False
    If hasMarker
        baseIs = Marker.GetBaseObject() == MarkerBase
    EndIf
    Debug.Trace("KKQ-Q02 marker-set=" + hasMarker + " base-is=" + baseIs)
    SetStage(10)
    Debug.Trace("KKQ-Q04 stage10-done=" + GetStageDone(10))
EndEvent
