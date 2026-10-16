ScriptName KitKraft:SiteClaim extends ObjectReference
{KitKraft prototype FTH-EXP-03: a claim controller on an activator reference.
The first activation by the player claims the site: Claimed goes from 0 to 1, the player
joins OwnerFaction, and the reference's parent cell gets OwnerFaction as its owner. Every
later activation changes nothing (idempotent). Declarations from `kit papyrus events`:
ObjectReference.psc OnActivate(ObjectReference) and GetParentCell(), GlobalVariable.psc
GetValue() and SetValue(float), Cell.psc SetFactionOwner(Faction), Actor.psc
AddToFaction(Faction), Game.psc GetPlayer(), Debug.psc Trace and Notification.}

GlobalVariable Property Claimed Auto Const Mandatory
{KitKraftSiteClaimed: 0 unclaimed, 1 claimed. The probes read it with getglobalvalue.}

Faction Property OwnerFaction Auto Const Mandatory
{The faction that owns the claimed site.}

String Property Label = "KitKraft site" Auto Const
{Shown in the notification and the Papyrus log lines the probes match.}

Auto State Unclaimed
    Event OnActivate(ObjectReference akActionRef)
        ; Leave the state before any call that can yield, so that a second activation
        ; queued behind this one lands in Claimed (or Busy) and changes nothing.
        GoToState("Busy")
        If akActionRef != Game.GetPlayer()
            GoToState("Unclaimed")
            Return
        EndIf
        If Claimed.GetValue() != 0.0
            ; Claimed already (an earlier save, or the console): keep the global as it is.
            Debug.Trace("KitKraftSiteClaim: " + Label + " already claimed, nothing changed")
            GoToState("Claimed")
            Return
        EndIf
        Claimed.SetValue(1.0)
        Game.GetPlayer().AddToFaction(OwnerFaction)
        GetParentCell().SetFactionOwner(OwnerFaction)
        Debug.Trace("KitKraftSiteClaim: " + Label + " claimed")
        Debug.Notification(Label + " claimed")
        GoToState("Claimed")
    EndEvent
EndState

State Busy
    Event OnActivate(ObjectReference akActionRef)
        Debug.Trace("KitKraftSiteClaim: " + Label + " claim in progress, activation ignored")
    EndEvent
EndState

State Claimed
    Event OnActivate(ObjectReference akActionRef)
        Debug.Trace("KitKraftSiteClaim: " + Label + " already claimed, nothing changed")
    EndEvent
EndState
