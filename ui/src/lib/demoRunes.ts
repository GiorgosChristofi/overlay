// Rune trees of patch 16.19.1 (Data Dragon) and Lux's mid rune pages from a
// recorded OP.GG response, for the demo scene and component tests. The app
// itself gets both from the core.
import type { RuneCatalogDto } from "../bindings/RuneCatalogDto";
import type { RunePageDto } from "../bindings/RunePageDto";

const page = (
  primaryStyleId: number,
  subStyleId: number,
  perkIds: number[],
  games: number,
  winRate: number,
  pickRate: number,
): RunePageDto => ({ primaryStyleId, subStyleId, perkIds, games, winRate, pickRate });

export const demoRunePages: RunePageDto[] = [
  page(8200, 8100, [8229, 8226, 8210, 8237, 8106, 8126, 5008, 5008, 5001], 4492, 0.5171, 0.4801),
  page(8100, 8200, [8128, 8126, 8140, 8106, 8210, 8226, 5008, 5008, 5001], 1050, 0.499, 0.1123),
  page(8200, 8000, [8229, 8226, 8210, 8237, 8009, 8014, 5008, 5008, 5001], 848, 0.4882, 0.0906),
  page(8200, 8300, [8229, 8226, 8210, 8237, 8345, 8347, 5008, 5008, 5001], 51, 0.5098, 0.0055),
  page(8300, 8200, [8369, 8321, 8313, 8347, 8210, 8226, 5008, 5008, 5001], 18, 0.4444, 0.0019),
];

export const demoRuneCatalog: RuneCatalogDto = {
  "trees": [
    {
      "id": 8100,
      "name": "Domination",
      "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/7200_Domination.png",
      "slots": [
        [
          {
            "id": 8112,
            "name": "Electrocute",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Domination/Electrocute/Electrocute.png",
            "description": "Hitting a champion with 3 separate attacks or abilities within 3s deals bonus adaptive damage.\n\nDamage: 70 - 240 (+0.1 bonus AD, +0.05 AP) damage.\nCooldown: 20s\n\n'We called them the Thunderlords, for to speak of their lightning was to invite disaster.'"
          },
          {
            "id": 8128,
            "name": "Dark Harvest",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Domination/DarkHarvest/DarkHarvest.png",
            "description": "Damaging a Champion below 50% health deals adaptive damage and harvests their soul, permanently increasing Dark Harvest's damage by 11.\n\nDark Harvest damage: 30 (+11 damage per soul) (+0.1 bonus AD) (+0.05 AP)\nCooldown: 35s (resets to 1.0s on takedown)"
          },
          {
            "id": 9923,
            "name": "Hail of Blades",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Domination/HailOfBlades/HailOfBlades.png",
            "description": "Gain 90% (60% for ranged champions) Attack Speed and bonus true damage when you attack an enemy champion for up to 3 attacks.\n\nNo more than 3s can elapse between attacks or this effect will end.\n\nCooldown: 10s.\nOn-Hit Damage: 2 - 20 (+0.12 bonus AD, +0.1 AP) damage.\n\nAttack resets increase the attack limit by 1.\nAllows you to temporarily exceed the Attack Speed limit."
          }
        ],
        [
          {
            "id": 8126,
            "name": "Cheap Shot",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Domination/CheapShot/CheapShot.png",
            "description": "Damaging champions with impaired movement or actions deals 10 - 45 bonus true damage (based on level).\n\nCooldown: 4s\nActivates on damage occurring after the impairment."
          },
          {
            "id": 8139,
            "name": "Taste of Blood",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Domination/TasteOfBlood/GreenTerror_TasteOfBlood.png",
            "description": "Heal when you damage an enemy champion.\n\nHealing: 16-40 (+0.1 bonus AD, +0.05 AP) health (based on level)\n\nCooldown: 20s"
          },
          {
            "id": 8143,
            "name": "Sudden Impact",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Domination/SuddenImpact/SuddenImpact.png",
            "description": "Damaging basic attacks and abilities deal a bonus 20 - 80 True Damage based on level to enemy champions after using a dash, leap, blink, teleport, or when leaving stealth for 4s.\n\nCooldown: 10s"
          }
        ],
        [
          {
            "id": 8137,
            "name": "Sixth Sense",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Domination/SixthSense/SixthSense.png",
            "description": "Automatically sense a nearby untracked and unseen ward, tracking it for the team.\n\nLevel 11: Also reveal the ward for 10s.\n\nThis effect has a 250 second Cooldown."
          },
          {
            "id": 8140,
            "name": "Grisly Mementos",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Domination/GrislyMementos/GrislyMementos.png",
            "description": "Collect 1 memento on champion takedowns, up to 18 total.\n\nGain 6 Trinket Haste for each collected. In game modes where vision Trinkets do not exist, instead gain 3 Summoner Spell Haste."
          },
          {
            "id": 8141,
            "name": "Deep Ward",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Domination/DeepWard/DeepWard.png",
            "description": "Your wards in the enemy jungle are Deep. Deep wards gain +1 extra Health and +[30 - 45]s increased duration (+[45 - 150]s for Trinket stealth wards).\n\nLevel 9: Wards in the river are also Deep."
          }
        ],
        [
          {
            "id": 8135,
            "name": "Treasure Hunter",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Domination/TreasureHunter/TreasureHunter.png",
            "description": "Gain an additional 50 gold the next time you collect a Bounty Hunter stack. Increase the gold gained by 20 gold for each Bounty Hunter stack, up to 130 gold.\n\nBounty Hunter stacks are earned the first time you get a takedown on each enemy champion."
          },
          {
            "id": 8105,
            "name": "Relentless Hunter",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Domination/RelentlessHunter/RelentlessHunter.png",
            "description": "Gain 8 Move Speed out of combat per Bounty Hunter stack. Bounty Hunter stacks are earned the first time you get a takedown on each enemy champion."
          },
          {
            "id": 8106,
            "name": "Ultimate Hunter",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Domination/UltimateHunter/UltimateHunter.png",
            "description": "Your ultimate gains 6 Ability Haste, plus an additional 5 Ability Haste per Bounty Hunter stack. Bounty Hunter stacks are earned the first time you get a takedown on each enemy champion."
          }
        ]
      ]
    },
    {
      "id": 8300,
      "name": "Inspiration",
      "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/7203_Whimsy.png",
      "slots": [
        [
          {
            "id": 8351,
            "name": "Glacial Augment",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Inspiration/GlacialAugment/GlacialAugment.png",
            "description": "Immobilizing an enemy champion will cause 3 glacial rays to emanate from them towards you and other nearby champions, creating frozen zones for 3 (+ the immobilizing effect's duration) seconds that slow enemies for 20% (+90% per 100% Heal and Shield Power) (+6% per 100 Ability Power) (+7% per 100 bonus Attack Damage) and reduce their damage by 15% against your allies (not including yourself).\n\nCooldown: 25s"
          },
          {
            "id": 8360,
            "name": "Unsealed Spellbook",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Inspiration/UnsealedSpellbook/UnsealedSpellbook.png",
            "description": "Swap one of your equipped Summoner Spells to a new, single use Summoner Spell. Each unique Summoner Spell you swap to permanently decreases your swap cooldown by 25s (initial swap cooldown is @f3@ seconds).\n\nYour first swap becomes available at 6 mins.\n\nSummoner Spells can only be swapped while out of combat.\nAfter using a swapped Summoner Spell you must swap 3 more times before the first can be selected again.\nSmite damage increases after two Summoner Spell swaps."
          },
          {
            "id": 8369,
            "name": "First Strike",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Inspiration/FirstStrike/FirstStrike.png",
            "description": "Attacks or abilities against an enemy champion within 0.25s of entering champion combat grant 10 gold and First Strike for 3 seconds, causing you to deal 7% extra damage against champions, and granting 50% (35% for ranged champions) of bonus damage dealt as gold.\n\nCooldown: 25 - 15s"
          }
        ],
        [
          {
            "id": 8306,
            "name": "Hextech Flashtraption",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Inspiration/HextechFlashtraption/HextechFlashtraption.png",
            "description": "While Flash is on cooldown it is replaced by Hexflash.\n\nHexflash: Channel for 2s to blink to a new location.\n\nCooldown: 20s. Goes on a 10s cooldown when you enter champion combat."
          },
          {
            "id": 8304,
            "name": "Magical Footwear",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Inspiration/MagicalFootwear/MagicalFootwear.png",
            "description": "You get free Slightly Magical Footwear at 12 min, but you cannot buy boots before then. For each takedown you acquire the boots 45s sooner.\n\nSlightly Magical Footwear grants you an additional 10 Move Speed."
          },
          {
            "id": 8321,
            "name": "Cash Back",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Inspiration/CashBack/CashBack2.png",
            "description": "Get 7.5% Gold back when you purchase Legendary Items."
          }
        ],
        [
          {
            "id": 8313,
            "name": "Triple Tonic",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Inspiration/PerfectTiming/AlchemistCabinet.png",
            "description": "Upon reaching level 3, gain an Elixir of Avarice.\nUpon reaching level 6, gain an Elixir of Force.\nUpon reaching level 9, gain an Elixir of Skill."
          },
          {
            "id": 8352,
            "name": "Time Warp Tonic",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Inspiration/TimeWarpTonic/TimeWarpTonic.png",
            "description": "Consuming a potion grants 40% of its health restoration immediately."
          },
          {
            "id": 8345,
            "name": "Biscuit Delivery",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Inspiration/BiscuitDelivery/BiscuitDelivery.png",
            "description": "Biscuit Delivery: Gain a Total Biscuit of Everlasting Will every 2 mins, until 6 min.\n\nBiscuits restore 20 + 2% of your maximum health, increased by up to 100% based on missing Health. Consuming or selling a Biscuit permanently increases your max health by 30."
          }
        ],
        [
          {
            "id": 8347,
            "name": "Cosmic Insight",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Inspiration/CosmicInsight/CosmicInsight.png",
            "description": "+18 Summoner Spell Haste\n+10 Item Haste"
          },
          {
            "id": 8410,
            "name": "Approach Velocity",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Resolve/ApproachVelocity/ApproachVelocity.png",
            "description": "Gain 7.5% Move Speed towards nearby enemy champions that are movement impaired. This bonus is increased to 15% Move Speed for any enemy champion that you impair.\n\nActivation Range for CC from allies: 1000"
          },
          {
            "id": 8316,
            "name": "Jack Of All Trades",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Inspiration/JackOfAllTrades/JackofAllTrades2.png",
            "description": "For each different stat gained from items, gain one Jack stack. Each stack grants you 1 Ability Haste.\n\nGain 8 or 20 bonus Adaptive Force at 5 and 10 stacks, respectively."
          }
        ]
      ]
    },
    {
      "id": 8000,
      "name": "Precision",
      "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/7201_Precision.png",
      "slots": [
        [
          {
            "id": 8005,
            "name": "Press the Attack",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Precision/PressTheAttack/PressTheAttack.png",
            "description": "Hitting an enemy champion with 3 consecutive basic attacks deals 40 - 160 bonus adaptive damage (based on level) and amplifies your damage dealt by 8% until you leave combat with champions."
          },
          {
            "id": 8008,
            "name": "Lethal Tempo",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Precision/LethalTempo/LethalTempoTemp.png",
            "description": "Attacking an enemy champion grants you [6% Melee || 4% Ranged] Attack Speed for 6 seconds, up to 6. At max stacks, deal [9 - 30 Melee || 6 - 24 Ranged] bonus adaptive damage On-Attack, increased by 1% per 1% Bonus Attack Speed."
          },
          {
            "id": 8021,
            "name": "Fleet Footwork",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Precision/FleetFootwork/FleetFootwork.png",
            "description": "Attacking and moving builds Energy stacks. At 100 stacks, your next attack is Energized\n\nEnergized attacks heal you for 15 - 160 (+0.1 Bonus AD, +0.05 AP) and grant 20% Move Speed for 1s.\n\nFor Ranged Champions, Healing is 60% effective and Move Speed is 75% effective. All healing is 15% effective against Minions."
          },
          {
            "id": 8010,
            "name": "Conqueror",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Precision/Conqueror/Conqueror.png",
            "description": "Basic attacks or spells that deal damage to an enemy champion grant 2 stacks of Conqueror for 5s, gaining 1.8-4 Adaptive Force per stack. Stacks up to 12 times. Ranged champions gain only 1 stack per basic attack.\n\nWhen fully stacked, heal for 8% of the damage you deal to champions (5% for ranged champions)."
          }
        ],
        [
          {
            "id": 9101,
            "name": "Absorb Life",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Precision/AbsorbLife/AbsorbLife.png",
            "description": "Killing a target heals you for @HealAmount@."
          },
          {
            "id": 9111,
            "name": "Triumph",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Precision/Triumph.png",
            "description": "Takedowns restore 5% of your missing health, 2.5% of your max health, and grant an additional 20 gold.\n\n'The most dangerous game brings the greatest glory.'\n—Noxian Reckoner"
          },
          {
            "id": 8009,
            "name": "Presence of Mind",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Precision/PresenceOfMind/PresenceOfMind.png",
            "description": "Damaging an enemy champion restores 6-50 (80% for ranged) mana or 6 energy.\n\nTakedowns restore 15% of your maximum mana or energy.\n\nCooldown for damage restoration: 8s"
          }
        ],
        [
          {
            "id": 9104,
            "name": "Legend: Alacrity",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Precision/LegendAlacrity/LegendAlacrity.png",
            "description": "Gain 3% attack speed plus an additional 1.5% for every Legend stack (max 10 stacks).\n\nEarn progress toward Legend stacks for every champion takedown, epic monster takedown, large monster kill, and minion kill."
          },
          {
            "id": 9105,
            "name": "Legend: Haste",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Precision/LegendHaste/LegendHaste.png",
            "description": "Gain 1.5 basic ability haste for every Legend stack (max 10 stacks).\n\nEarn progress toward Legend stacks for every champion takedown, epic monster takedown, large monster kill, and minion kill."
          },
          {
            "id": 9103,
            "name": "Legend: Bloodline",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Precision/LegendBloodline/LegendBloodline.png",
            "description": "Gain 0.45% Life Steal for every Legend stack (max 15 stacks). At maximum Legend stacks, gain 85 max health.\n\nEarn progress toward Legend stacks for every champion takedown, epic monster takedown, large monster kill, and minion kill."
          }
        ],
        [
          {
            "id": 8014,
            "name": "Coup de Grace",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Precision/CoupDeGrace/CoupDeGrace.png",
            "description": "Deal 8% more damage to champions who have less than 40% health."
          },
          {
            "id": 8017,
            "name": "Cut Down",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Precision/CutDown/CutDown.png",
            "description": "Deal 8% more damage to champions who have more than 60% health."
          },
          {
            "id": 8299,
            "name": "Last Stand",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Sorcery/LastStand/LastStand.png",
            "description": "Deal 5% - 11% increased damage to champions while you are below 60% health. Max damage gained at 30% health."
          }
        ]
      ]
    },
    {
      "id": 8400,
      "name": "Resolve",
      "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/7204_Resolve.png",
      "slots": [
        [
          {
            "id": 8437,
            "name": "Grasp of the Undying",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Resolve/GraspOfTheUndying/GraspOfTheUndying.png",
            "description": "Every 4s in combat, your next basic attack on a champion will:\nDeal bonus magic damage equal to 3.5% of your max health\nHeal you for 1.3% of your max health\nPermanently increase your health by 5\nRanged Champions: Damage, healing, and permanent health gained are 40% effective."
          },
          {
            "id": 8439,
            "name": "Aftershock",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Resolve/VeteranAftershock/VeteranAftershock.png",
            "description": "After immobilizing an enemy champion, increase your Armor and Magic Resist by 45 + 75% of your Bonus Resists for 2.5s. Then explode, dealing magic damage to nearby enemies.\n\nDamage: 25 - 120 (+8% of your bonus health)\nCooldown: 20s\n\nResistance bonus from Aftershock capped at: 80-150 (based on level)"
          },
          {
            "id": 8465,
            "name": "Guardian",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Resolve/Guardian/Guardian.png",
            "description": "Guard allies within 350 units of you, and allies you target with spells for 2.5s. While Guarding, if you or the ally take more than a small amount of damage over the duration of the Guard, both of you gain a shield for 1.5s.\n\nCooldown: 75 - 40 seconds\nShield: 40 - 150 + 20% of your ability power + 6% of your bonus health\nProc Threshold: 50 - 165 postmitigation damage"
          }
        ],
        [
          {
            "id": 8446,
            "name": "Demolish",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Resolve/Demolish/Demolish.png",
            "description": "Your third attack against towers deals [85 (+28% max Health) Melee || 50 (+20% max Health) Ranged] bonus physical damage.\n\nCooldown: 30s"
          },
          {
            "id": 8463,
            "name": "Font of Life",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Resolve/FontOfLife/FontOfLife.png",
            "description": "Impairing the movement of an enemy champion restores @BaseHeal@ Health to you and the lowest health nearby allied champion.\n\n70% effect for Ranged Users.\n\nCooldown: 20s"
          },
          {
            "id": 8401,
            "name": "Shield Bash",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Resolve/MirrorShell/MirrorShell.png",
            "description": "Whenever you gain a new shield, your next basic attack against a champion deals 5 - 30 (+2.5% Bonus Health) (+15.0% New Shield Amount) bonus adaptive damage.\n\nYou have up to 2s after the shield expires to use this effect."
          }
        ],
        [
          {
            "id": 8429,
            "name": "Conditioning",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Resolve/Conditioning/Conditioning.png",
            "description": "After 12 min gain +8 Armor and +8 Magic Resist and increase your Armor and Magic Resist by 3%."
          },
          {
            "id": 8444,
            "name": "Second Wind",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Resolve/SecondWind/SecondWind.png",
            "description": "After taking damage from an enemy champion, heal for 4% of your missing health over 10s."
          },
          {
            "id": 8473,
            "name": "Bone Plating",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Resolve/BonePlating/BonePlating.png",
            "description": "After taking damage from an enemy champion, the next 3 spells or attacks you receive from them deal 30-60 (based on level) less damage.\n\nDuration: 1.5s\nCooldown: 55s"
          }
        ],
        [
          {
            "id": 8451,
            "name": "Overgrowth",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Resolve/Overgrowth/Overgrowth.png",
            "description": "Absorb life essence from monsters or enemy minions that die near you, permanently gaining 3 maximum health for every 8.\n\nWhen you've absorbed 120 monsters or enemy minions, gain an additional 3.5% maximum health."
          },
          {
            "id": 8453,
            "name": "Revitalize",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Resolve/Revitalize/Revitalize.png",
            "description": "Gain 5% Heal and Shield Power.\n\nHeals and shields you cast or receive are 10% stronger on targets below 40% health."
          },
          {
            "id": 8242,
            "name": "Unflinching",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Sorcery/Unflinching/Unflinching.png",
            "description": "Gain 10 Armor and Magic Resist when crowd controlled and for 2 seconds after."
          }
        ]
      ]
    },
    {
      "id": 8200,
      "name": "Sorcery",
      "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/7202_Sorcery.png",
      "slots": [
        [
          {
            "id": 8214,
            "name": "Summon Aery",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Sorcery/SummonAery/SummonAery.png",
            "description": "Damaging enemy champions with basic attacks or abilities sends Aery to them, dealing 10 - 50 based on level (+0.05 AP) (+0.1 bonus AD).\n\nEmpowering or protecting allies with abilities sends Aery to them, shielding them for 20 - 100 based on level (+0.05 AP) (+0.1 bonus AD).\n\nAery cannot be sent out again until she returns to you."
          },
          {
            "id": 8229,
            "name": "Arcane Comet",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Sorcery/ArcaneComet/ArcaneComet.png",
            "description": "Damaging a champion with an ability hurls a comet at their location, dealing increased damage based on distance.\n\nAdaptive Damage: 15 - 100 based on level (+0.05 AP and +0.1 bonus AD)\nCooldown: 20 - 8s\n\nDamage amplification scales up to 100% at 750 range."
          },
          {
            "id": 8230,
            "name": "Stormraider's Surge",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Sorcery/PhaseRush/StormraidersSurgeRuneIcon2.png",
            "description": "Dealing 25% of a champion's maximum health within 3s grants 48% Move Speed and 50% Slow Resistance for 4s. Move Speed is 75% effective for ranged champions.\nCooldown: 20s - 10s"
          },
          {
            "id": 8992,
            "name": "Deathfire Touch",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Sorcery/DeathfireTouch/DEATHFIRE_TOUCH_KEYSTONE.png",
            "description": "Damaging a champion with an ability burns them for 3 - 12 based on level (+2.5% AP) (+7% bonus AD) magic damage per second. After burning for 3 seconds, the damage of the burn increases by 75% while they remain on fire.\n\nDuration:\nSingle Target: 4s\nArea of Effect: 2s\nDamage over Time: 1s"
          }
        ],
        [
          {
            "id": 8224,
            "name": "Axiom Arcanist",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Sorcery/NullifyingOrb/Axiom_Arcanist.png",
            "description": "Your Ultimate has 12% increased damage, healing, and shielding.\n(AoE damage is reduced to a 8% increase)\n\nScoring a takedown on an enemy champion reduces your Ultimate's current cooldown by 7%."
          },
          {
            "id": 8226,
            "name": "Manaflow Band",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Sorcery/ManaflowBand/ManaflowBand.png",
            "description": "Hitting an enemy champion with an ability permanently increases your maximum mana by 25, up to 250 mana.\n\nAfter reaching 250 bonus mana, restore 1% of your missing mana every 5 seconds.\n\nCooldown: 15 seconds"
          },
          {
            "id": 8275,
            "name": "Nimbus Cloak",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Sorcery/NimbusCloak/6361.png",
            "description": "After casting a Summoner Spell, gain a Move Speed increase that lasts for 2s and allows you to pass through units.\n\nIncrease: 15% - 45% Move Speed based on the Summoner Spell's cooldown. (Higher cooldown Summoner Spells grant more Move Speed)."
          }
        ],
        [
          {
            "id": 8210,
            "name": "Transcendence",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Sorcery/Transcendence/Transcendence.png",
            "description": "Gain bonuses upon reaching the following levels:\nLevel 5: +5 Ability Haste\nLevel 8: +5 Ability Haste\nLevel 11: On Champion takedown, reduce the remaining cooldown of basic abilities by 20%."
          },
          {
            "id": 8234,
            "name": "Celerity",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Sorcery/Celerity/CelerityTemp.png",
            "description": "All movement bonuses are 7% more effective on you and gain 1% Move Speed."
          },
          {
            "id": 8233,
            "name": "Absolute Focus",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Sorcery/AbsoluteFocus/AbsoluteFocus.png",
            "description": "While above 70% health, gain an adaptive bonus of up to 18 Attack Damage or 30 Ability Power (based on level).\n\nGrants 1.8 Attack Damage or 3 Ability Power at level 1."
          }
        ],
        [
          {
            "id": 8237,
            "name": "Scorch",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Sorcery/Scorch/Scorch.png",
            "description": "Your next damaging ability hit sets champions on fire dealing 20 - 40 bonus magic damage based on level after 1s.\n\nCooldown: 10s"
          },
          {
            "id": 8232,
            "name": "Waterwalking",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Sorcery/Waterwalking/Waterwalking.png",
            "description": "Gain 10 Move Speed and 13 - 30 Adaptive Force (based on level) when in the river.\n\nMay you be as swift as the rushing river and agile as a startled Rift Scuttler."
          },
          {
            "id": 8236,
            "name": "Gathering Storm",
            "iconUrl": "https://ddragon.leagueoflegends.com/cdn/img/perk-images/Styles/Sorcery/GatheringStorm/GatheringStorm.png",
            "description": "Every 10 min gain AP or AD, adaptive.\n\n10 min: + 8 AP or 5 AD\n20 min: + 24 AP or 14 AD\n30 min: + 48 AP or 29 AD\n40 min: + 80 AP or 48 AD\n50 min: + 120 AP or 72 AD\n60 min: + 168 AP or 101 AD\netc..."
          }
        ]
      ]
    }
  ],
  "statShardRows": [
    [
      {
        "id": 5008,
        "name": "Adaptive Force",
        "iconUrl": "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/perk-images/statmods/statmodsadaptiveforceicon.png",
        "description": "+9 Adaptive Force (5.4 bonus AD or 9 AP)"
      },
      {
        "id": 5005,
        "name": "Attack Speed",
        "iconUrl": "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/perk-images/statmods/statmodsattackspeedicon.png",
        "description": "+10% Attack Speed"
      },
      {
        "id": 5007,
        "name": "Ability Haste",
        "iconUrl": "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/perk-images/statmods/statmodscdrscalingicon.png",
        "description": "+8 Ability Haste"
      }
    ],
    [
      {
        "id": 5008,
        "name": "Adaptive Force",
        "iconUrl": "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/perk-images/statmods/statmodsadaptiveforceicon.png",
        "description": "+9 Adaptive Force (5.4 bonus AD or 9 AP)"
      },
      {
        "id": 5010,
        "name": "Move Speed",
        "iconUrl": "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/perk-images/statmods/statmodsmovementspeedicon.png",
        "description": "+2.5% Move Speed"
      },
      {
        "id": 5001,
        "name": "Health Scaling",
        "iconUrl": "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/perk-images/statmods/statmodshealthplusicon.png",
        "description": "+10-180 Health (based on level)"
      }
    ],
    [
      {
        "id": 5011,
        "name": "Health",
        "iconUrl": "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/perk-images/statmods/statmodshealthscalingicon.png",
        "description": "+65 Health"
      },
      {
        "id": 5013,
        "name": "Tenacity and Slow Resist",
        "iconUrl": "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/perk-images/statmods/statmodstenacityicon.png",
        "description": "+15% Tenacity and Slow Resist"
      },
      {
        "id": 5001,
        "name": "Health Scaling",
        "iconUrl": "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/perk-images/statmods/statmodshealthplusicon.png",
        "description": "+10-180 Health (based on level)"
      }
    ]
  ]
};
