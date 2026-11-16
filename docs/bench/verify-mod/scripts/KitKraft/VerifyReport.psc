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

Function Present(String row, String type, Int objectId)
    {Presence of a form `help` cannot find (game-20261116-04: the game keeps no EditorID of
    the type and the form has no FULL, or its FULL was not listed): the form the plugin
    defines at objectId loaded. Presence only: no field of it is read.}
    Report(row, type + " present=" + (Game.GetFormFromFile(objectId, PluginName) != None))
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
    ; Presence rows V-H-<type> for the forms `help` found nothing for (object IDs as in
    ; verify-mod.toml and make_probes.py EXPECTED).
    Present("H20", "PKIN", 0x100A)
    Present("H43", "OTFT", 0x1015)
    Present("H51", "SOUN", 0x1017)
    Present("H52", "IRES", 0x1018)
    Present("H53", "TXST", 0x1019)
    Present("H55", "LVLN", 0x101B)
    Present("H56", "LVLP", 0x101C)
    Present("H57", "PACK", 0x101D)
    Present("H59", "WKMF", 0x101F)
    Present("H62", "COBJ", 0x1022)
    Present("H63", "REGN", 0x1023)
    Present("H64", "PCBN", 0x1024)
    Present("H65", "PCCN", 0x1025)
    Present("H66", "SFBK", 0x1026)
    Present("H67", "SFPT", 0x1027)
    Present("H68", "SFTR", 0x1028)
    Present("H01", "GLOB", 0x1029)
    Present("H02", "FLST", 0x102A)
    Present("H04", "QUST", 0x102C)
    Present("H05", "LCTN", 0x102D)
    Report("P99", "done")
EndEvent
