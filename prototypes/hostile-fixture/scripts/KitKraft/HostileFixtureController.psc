ScriptName KitKraft:HostileFixtureController extends ObjectReference
{KitKraft M5.H2 prototype (hostile-encounter fixture, first half). While this reference
is loaded: the quest's spawn stage (or TriggerGlobal set to 1) places SpawnCount actors of
SpawnBase here unless the cooldown global says not yet; the cleanup stage (or TriggerGlobal
set to 2) deletes them. Every step is traced to the Papyrus log for the probes.}

Quest Property FixtureQuest Auto Const
{The quest whose stages drive the fixture (none: the global trigger only).}
Int Property SpawnStage = 10 Auto Const
Int Property CleanupStage = 20 Auto Const
ActorBase Property SpawnBase Auto Const
{The actor base to place (an NPC_ templated on a leveled list).}
Int Property SpawnCount = 2 Auto Const
GlobalVariable Property SpawnedGlobal Auto Const
{Telemetry: how many placed actors the fixture holds now.}
GlobalVariable Property NextDayGlobal Auto Const
{Cooldown: the game time (days) before which a spawn is refused.}
GlobalVariable Property TriggerGlobal Auto Const
{Fallback trigger, polled while loaded: 1 spawns, 2 cleans up; reset to 0 when read.}
Float Property CooldownDays = 0.0 Auto Const
Float Property PollSeconds = 1.0 Auto Const

Actor[] Spawned

Event OnLoad()
    If FixtureQuest
        RegisterForRemoteEvent(FixtureQuest, "OnStageSet")
    EndIf
    StartTimer(PollSeconds, 1)
    Debug.Trace("KitKraftHostileFixture loaded")
EndEvent

Event OnUnload()
    ; Nonpunitive while the player is away: nothing placed is touched here.
    CancelTimer(1)
EndEvent

Event Quest.OnStageSet(Quest akSender, int auiStageID, int auiItemID)
    If auiStageID == SpawnStage
        Spawn()
    ElseIf auiStageID == CleanupStage
        Cleanup()
    EndIf
EndEvent

Event OnTimer(int aiTimerID)
    If aiTimerID != 1
        Return
    EndIf
    If TriggerGlobal
        Float request = TriggerGlobal.GetValue()
        If request == 1.0
            TriggerGlobal.SetValue(0.0)
            Spawn()
        ElseIf request == 2.0
            TriggerGlobal.SetValue(0.0)
            Cleanup()
        EndIf
    EndIf
    If Is3DLoaded()
        StartTimer(PollSeconds, 1)
    EndIf
EndEvent

Function Spawn()
    Float now = Utility.GetCurrentGameTime()
    If NextDayGlobal && now < NextDayGlobal.GetValue()
        Debug.Trace("KitKraftHostileFixture cooldown " + now + " < " + NextDayGlobal.GetValue())
        Return
    EndIf
    If !Spawned
        Spawned = new Actor[0]
    EndIf
    Int i = 0
    While i < SpawnCount
        Actor placed = PlaceActorAtMe(SpawnBase)
        If placed
            Spawned.Add(placed)
        EndIf
        i += 1
    EndWhile
    If SpawnedGlobal
        SpawnedGlobal.SetValue(Spawned.Length as Float)
    EndIf
    If NextDayGlobal
        NextDayGlobal.SetValue(now + CooldownDays)
    EndIf
    Debug.Trace("KitKraftHostileFixture spawn " + Spawned.Length)
EndFunction

Function Cleanup()
    Int removed = 0
    If Spawned
        Int i = Spawned.Length
        While i > 0
            i -= 1
            Actor placed = Spawned[i]
            If placed
                placed.Disable()
                placed.Delete()
                removed += 1
            EndIf
        EndWhile
        Spawned.Clear()
    EndIf
    If SpawnedGlobal
        SpawnedGlobal.SetValue(0.0)
    EndIf
    Debug.Trace("KitKraftHostileFixture cleanup " + removed)
EndFunction
