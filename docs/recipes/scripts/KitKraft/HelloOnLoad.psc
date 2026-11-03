ScriptName KitKraft:HelloOnLoad extends ObjectReference
{KitKraft M4.E9 recipe: shows a notification when the reference it is attached to loads,
and (M4.T6) writes the same text to the Papyrus log once, so a game session can check it.
Event and functions from `kit papyrus events`: ObjectReference.psc OnLoad(),
Debug.psc Notification(string), Debug.psc Trace(string, int).}

String Property Text = "KitKraft: hello from a script attached by kit" Auto Const
{The notification text; the recipe sets it through VMAD.}

Int Property Times = 1 Auto Const
{How many notifications to show (1 or more).}

Event OnLoad()
    Debug.Trace(Text)
    Int i = 0
    While i < Times
        Debug.Notification(Text)
        i += 1
    EndWhile
EndEvent
