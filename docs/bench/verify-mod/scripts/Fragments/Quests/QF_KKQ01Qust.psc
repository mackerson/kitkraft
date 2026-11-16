ScriptName Fragments:Quests:QF_KKQ01Qust extends Quest Hidden Const
{KitKraft bench verification quest (docs/bench/verify-mod.md, "Self-reporting quest"): the
fragment script of KKQ01Qust. The recipe writes it into the quest's VMAD fragment block
with one stage fragment, stage 10, which runs Fragment_Stage_0010_Item_00.}

Function Fragment_Stage_0010_Item_00()
    Debug.Trace("KKQ-Q03 stage 10 fragment ran")
EndFunction
