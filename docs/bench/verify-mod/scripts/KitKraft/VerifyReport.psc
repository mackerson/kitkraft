ScriptName KitKraft:VerifyReport extends ObjectReference
{KitKraft bench verification mod (docs/bench/verify-mod.md): when the reference it is
attached to loads, it checks the forms the verification plugin wrote and writes one
Debug.Trace line per check to the Papyrus log, each starting with "KKV-P" and its probe
row, so a game session's papyrus-log rows can read every result on its own. A check whose
property is None writes its line with the value the call returns on None, so one missing
form fails one row, not the rest. Every function used is declared in the base game's
sources (kit papyrus events, 2026-11-15): GlobalVariable.GetValueInt, FormList.GetSize and
HasForm, Form.HasKeyword and GetGoldValue, ObjectReference.GetBaseObject, GetItemCount,
AddItem, GetCurrentLocation, GetValue and SetValue, Actor.AddToFaction, IsInFaction,
AddPerk and HasPerk, Game.GetFormFromFile, Quest.Start, SetStage, GetStageDone and IsRunning, Game.GetPlayer,
Debug.Trace.}

GlobalVariable Property Glob Auto Const
{KKV GLOB, FLTV 4242.}
FormList Property Flst Auto Const
{KKV FLST: LNAM the mug and the wrench.}
Form Property Mug Auto Const
{KKV MISC (the mug, DATA Value 777): first entry of the list, three in the container.}
Form Property Wrench Auto Const
{KKV MISC reached only through the leveled list.}
Keyword Property Kwd Auto Const
{KKV KYWD, in the reporter activator's KWDA.}
Faction Property Fact Auto Const
Perk Property PerkToAdd Auto Const
ActorValue Property Av Auto Const
Quest Property Qst Auto Const
{KKV QUST: stages 10 and 20, no script, not start-game-enabled.}
Location Property Loc Auto Const
{KKV LCTN: the XLCN of the apartment override.}
String Property PluginName = "KitKraftVerify.esm" Auto Const
{The plugin whose placed references RefIds, ChestId and PackInId name.}
Int Property ChestId Auto Const
{Object ID of the placed KKV CONT (CNTO: three mugs) in PluginName.}
Form Property LeveledList Auto Const
{KKV LVLI: one entry, the wrench, count 1.}
Int[] Property RefIds Auto Const
{Object IDs in PluginName of the placed references whose base is checked, in the order of
Bases and Labels (a script property cannot name a reference the same build places, so the
references are looked up by ID; verify-mod.md says how the IDs are checked).}
Form[] Property Bases Auto Const
String[] Property Labels Auto Const
{The probe row and type of each Refs entry, as "P20 STAT".}
Int Property PackInId Auto Const
{Object ID of the placed pack-in reference: its base is written out for a person to read.}

Function Report(String row, String text)
    Debug.Trace("KKV-" + row + " " + text)
EndFunction

Event OnLoad()
    Actor player = Game.GetPlayer()
    ObjectReference Chest = Game.GetFormFromFile(ChestId, PluginName) as ObjectReference
    Report("P00", "reporter loaded")
    Report("P01", "GLOB value=" + Glob.GetValueInt())
    Report("P02", "FLST size=" + Flst.GetSize())
    Report("P03", "FLST has-mug=" + Flst.HasForm(Mug) + " has-wrench=" + Flst.HasForm(Wrench))
    Report("P04", "MISC gold=" + Mug.GetGoldValue())
    Report("P05", "KYWD on-reporter=" + HasKeyword(Kwd))
    player.AddToFaction(Fact)
    Report("P06", "FACT player-in=" + player.IsInFaction(Fact))
    player.AddPerk(PerkToAdd)
    Report("P07", "PERK player-has=" + player.HasPerk(PerkToAdd))
    player.SetValue(Av, 7.0)
    Report("P08", "AVIF player-value=" + (player.GetValue(Av) as Int))
    Qst.Start()
    Qst.SetStage(10)
    Report("P09", "QUST running=" + Qst.IsRunning() + " stage10-done=" + Qst.GetStageDone(10))
    Report("P10", "LCTN reporter-in=" + (GetCurrentLocation() == Loc))
    Report("P11", "CONT mugs=" + Chest.GetItemCount(Mug))
    Int before = Chest.GetItemCount(Wrench)
    Chest.AddItem(LeveledList, 1, true)
    Report("P12", "LVLI wrench-added=" + (Chest.GetItemCount(Wrench) - before))
    Int i = 0
    While i < RefIds.Length && i < Bases.Length && i < Labels.Length
        ObjectReference r = Game.GetFormFromFile(RefIds[i], PluginName) as ObjectReference
        Report(Labels[i], "base-is=" + (r.GetBaseObject() == Bases[i]))
        i += 1
    EndWhile
    ; The pack-in question (game-20261103-08 row T-20): what the game reports as the base.
    ObjectReference packInRef = Game.GetFormFromFile(PackInId, PluginName) as ObjectReference
    Report("P98", "PKIN reference base=" + packInRef.GetBaseObject())
    Report("P99", "done")
EndEvent
