ScriptName KitKraft:VerifyAliasReport extends ReferenceAlias
{KitKraft bench verification quest (docs/bench/verify-mod.md, "Alias script"): attached to
reference alias 0 (MarkerAlias) of the quest KKQ01Qust of KitKraftVerifyQuest.esm, through
the alias section of the quest's VMAD fragment block. When the alias is filled at quest
start it writes one Debug.Trace line for probe row Q-05: that the alias script runs, that
its property was written, and that the alias holds the reference the same build placed.}

ObjectReference Property Expected Auto Const
{The placed reference KKQ70Marker (new:marker), which the alias is forced to.}
Int Property Count Auto Const
{7 in the recipe; the script's default is 0, so 7 shows the alias script's property was read.}

Event OnAliasInit()
    Debug.Trace("KKQ-Q05 alias script running count=" + Count + " ref-is-marker=" + (GetReference() == Expected))
EndEvent
