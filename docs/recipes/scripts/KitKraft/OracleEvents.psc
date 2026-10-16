ScriptName KitKraft:OracleEvents extends ObjectReference
{KitKraft papyrus oracle fixture (ours): events, states and state switches.}

Int Property Activations = 0 Auto

Event OnInit()
    Debug.Trace("OracleEvents: init")
EndEvent

Event OnLoad()
    GoToState("Ready")
EndEvent

Event OnActivate(ObjectReference akActionRef)
    Activations += 1
EndEvent

State Ready
    Event OnActivate(ObjectReference akActionRef)
        Activations += 2
        If akActionRef == Game.GetPlayer()
            GoToState("Done")
        EndIf
    EndEvent
EndState

Auto State Waiting
    Event OnUnload()
        Debug.Trace("OracleEvents: unloaded while waiting")
    EndEvent
EndState

State Done
    Event OnBeginState(String asOldState)
        Debug.Trace("OracleEvents: done after " + asOldState)
    EndEvent
EndState
