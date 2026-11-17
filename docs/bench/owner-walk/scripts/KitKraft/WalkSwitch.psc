ScriptName KitKraft:WalkSwitch extends ObjectReference
{KitKraft owner walk (docs/bench/owner-walk/README.md), station W-06: when the owner
activates the switch it is attached to, it adds one to a global, writes one Debug.Trace
line and shows a notification, so `OnActivate` leaves three traces: a value the console
reads (getglobalvalue), a Papyrus log line and text on screen. Every function used is
declared in the base game's sources: ObjectReference OnLoad and OnActivate,
GlobalVariable.GetValue, SetValue and GetValueInt, Game.GetPlayer, Debug.Trace and
Debug.Notification.}

GlobalVariable Property Counter Auto Const
{KKW02Glob: how many times the switch was activated (starts at 0).}

String Property Text = "KitKraft walk: the switch was pressed" Auto Const
{The notification text; the recipe sets it through VMAD.}

Event OnLoad()
    Debug.Trace("KKW-A00 walk switch loaded")
EndEvent

Event OnActivate(ObjectReference akActionRef)
    Counter.SetValue(Counter.GetValue() + 1.0)
    Bool byPlayer = akActionRef == Game.GetPlayer()
    Debug.Trace("KKW-A01 activated by-player=" + byPlayer + " count=" + Counter.GetValueInt())
    Debug.Notification(Text)
EndEvent
