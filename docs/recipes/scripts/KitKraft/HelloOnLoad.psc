ScriptName KitKraft:HelloOnLoad extends ObjectReference
{KitKraft M4.E9 recipe: shows a notification when the reference it is attached to loads.
Event and function from `kit papyrus events`: ObjectReference.psc OnLoad(),
Debug.psc Notification(string).}

String Property Text = "KitKraft: hello from a script attached by kit" Auto Const
{The notification text; the recipe sets it through VMAD.}

Int Property Times = 1 Auto Const
{How many notifications to show (1 or more).}

Event OnLoad()
    Int i = 0
    While i < Times
        Debug.Notification(Text)
        i += 1
    EndWhile
EndEvent
