# Partition first IRQ timing, issue 216

62,500 ticks = 1 ms. Tuples give opening to preceding IRQ sample, IRQ sample to selection, selection to return sample, return sample to first EL0 observation, and total. Delta is candidate minus base. BC rounds 0/1/4, CB rounds 2/3.

|Clock/table/round|Base|Candidate|Delta|Candidate margin at IRQ / selected / return / EL0|
|---|---|---|---|---|
|default/p/0|39425 / 40856 / 516 / 7084 / 87881|29419 / 34696 / 491 / 5907 / 70513|-10006 / -6160 / -25 / -1177 / -17368|33081 / -1615 / -2106 / -8013|
|default/p/1|27474 / 45642 / 612 / 7038 / 80766|26721 / 34635 / 552 / 6064 / 67972|-753 / -11007 / -60 / -974 / -12794|35779 / 1144 / 592 / -5472|
|default/p/2|25566 / 42380 / 867 / 8710 / 77523|27531 / 58016 / 936 / 9216 / 95699|1965 / 15636 / 69 / 506 / 18176|34969 / -23047 / -23983 / -33199|
|default/p/3|17955 / 48533 / 931 / 8407 / 75826|26250 / 41428 / 530 / 6169 / 74377|8295 / -7105 / -401 / -2238 / -1449|36250 / -5178 / -5708 / -11877|
|default/p/4|22654 / 36575 / 484 / 5491 / 65204|23190 / 34818 / 493 / 5416 / 63917|536 / -1757 / 9 / -75 / -1287|39310 / 4492 / 3999 / -1417|
|default/q/0|25994 / 39607 / 453 / 1874 / 67928|21029 / 40695 / 570 / 1606 / 63900|-4965 / 1088 / 117 / -268 / -4028|41471 / 776 / 206 / -1400|
|default/q/1|24181 / 36326 / 412 / 3663 / 64582|22840 / 56864 / 978 / 2468 / 83150|-1341 / 20538 / 566 / -1195 / 18568|39660 / -17204 / -18182 / -20650|
|default/q/2|20525 / 41957 / 845 / 2286 / 65613|32892 / 35697 / 570 / 1987 / 71146|12367 / -6260 / -275 / -299 / 5533|29608 / -6089 / -6659 / -8646|
|default/q/3|19338 / 42876 / 390 / 1804 / 64408|25906 / 39215 / 607 / 1651 / 67379|6568 / -3661 / 217 / -153 / 2971|36594 / -2621 / -3228 / -4879|
|default/q/4|16459 / 42373 / 353 / 1622 / 60807|16937 / 33297 / 484 / 1390 / 52108|478 / -9076 / 131 / -232 / -8699|45563 / 12266 / 11782 / 10392|
|icount3/p/0|2 / 996 / 419 / 30 / 1447|2 / 983 / 409 / 30 / 1424|0 / -13 / -10 / 0 / -23|62498 / 61515 / 61106 / 61076|
|icount3/p/1|2 / 996 / 419 / 30 / 1447|2 / 983 / 409 / 30 / 1424|0 / -13 / -10 / 0 / -23|62498 / 61515 / 61106 / 61076|
|icount3/p/2|2 / 996 / 419 / 30 / 1447|2 / 983 / 409 / 30 / 1424|0 / -13 / -10 / 0 / -23|62498 / 61515 / 61106 / 61076|
|icount3/p/3|2 / 996 / 419 / 30 / 1447|2 / 983 / 409 / 30 / 1424|0 / -13 / -10 / 0 / -23|62498 / 61515 / 61106 / 61076|
|icount3/p/4|2 / 996 / 419 / 30 / 1447|2 / 983 / 409 / 30 / 1424|0 / -13 / -10 / 0 / -23|62498 / 61515 / 61106 / 61076|
|icount3/q/0|2 / 1019 / 418 / 13 / 1452|2 / 1005 / 409 / 13 / 1429|0 / -14 / -9 / 0 / -23|62498 / 61493 / 61084 / 61071|
|icount3/q/1|2 / 1019 / 418 / 13 / 1452|2 / 1005 / 409 / 13 / 1429|0 / -14 / -9 / 0 / -23|62498 / 61493 / 61084 / 61071|
|icount3/q/2|2 / 1019 / 418 / 13 / 1452|2 / 1005 / 409 / 13 / 1429|0 / -14 / -9 / 0 / -23|62498 / 61493 / 61084 / 61071|
|icount3/q/3|2 / 1019 / 418 / 13 / 1452|2 / 1005 / 409 / 13 / 1429|0 / -14 / -9 / 0 / -23|62498 / 61493 / 61084 / 61071|
|icount3/q/4|2 / 1019 / 418 / 13 / 1452|2 / 1005 / 409 / 13 / 1429|0 / -14 / -9 / 0 / -23|62498 / 61493 / 61084 / 61071|

Later IRQ transitions by clock, same PID, extension. Generation 1 has no preceding IRQ.

|Group|N|Min..max for each stage and total|
|---|---:|---|
|('default', False, False)|140|6761..22995 / 1368..10976 / 244..2514 / 514..15592 / 10133..43519|
|('default', False, True)|10|8653..16494 / 1818..3730 / 340..705 / 1120..5811 / 12523..24700|
|('default', True, False)|30|7176..14359 / 1100..8868 / 138..436 / 922..3728 / 9809..24138|
|('default', True, True)|10|7098..16427 / 1310..2926 / 121..412 / 941..2578 / 9827..22214|
|('icount3', False, False)|140|2..2 / 979..1012 / 409..419 / 10..32 / 1406..1462|
|('icount3', False, True)|10|2..2 / 993..1007 / 409..418 / 14..17 / 1421..1441|
|('icount3', True, False)|30|2..2 / 987..1022 / 409..419 / 10..14 / 1409..1453|
|('icount3', True, True)|10|2..2 / 987..1000 / 409..418 / 15..16 / 1413..1436|

Default generation 2 is A→idle for P (owner A→B, B unreleased) and idle→idle for Q (owner B→unassigned). Both have a first-IRQ i→selection interval of 33,297–58,016 across the combined sample versus 1,100–10,976 across later IRQs. P also has a longer return→EL0 interval (5,416–9,216 versus Q 1,390–3,663). No individual interval exhausts the budget. Of 20 default first IRQs, 11 exceed it by selection, seven cross later and two pass. P misses 10/10 and Q 8/10. Candidate totals improve in six default pairs and regress in four. Icount3 improves by 23 ticks every pair with unchanged opening→IRQ and return→EL0.

Keep #216 open. At the pinned source, the interval from IRQ sample to selection covers previous frame observation, result reads, evidence writes and checks, scheduling, MMU switching and generation publication. Before changing another operation, bracket these groups on first and later IRQs, then confirm any improvement using the original instrumentation and counterbalanced normal first boots. Current timestamps cannot apportion time among those operations or identify a host translation/cache effect.

기본 클록에서는 첫 IRQ에서 P가 A에서 idle로 넘어가고 Q는 idle을 계속 실행한다. 두 경우 모두 IRQ 관측에서 선택까지 후속 IRQ보다 오래 걸린다. 20건 중 11건은 선택 시점에 이미 1 ms를 넘었고 7건은 그 뒤에 초과했다. icount3에서 모든 쌍이 23틱 개선됐으나 기본 클록은 6쌍 개선, 4쌍 악화다. #216은 열린 상태로 유지한다. 관측·검증·디스패치 내부를 분리 계측한 뒤 원래 계측 조건의 첫 부팅 쌍으로 개선을 검증한다.

Archive SHA-256 c24226ba7fa7333f406d7ff8ab020e76a255ccb1a67ec1a412e68914d3880d6a. [Pinned source](https://github.com/Romanof123/JNOX/blob/01521256ef1052d07a9fc73b82f7b38af0a70a00/kernel/src/native_partition.rs#L94-L236)
