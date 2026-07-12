#!/bin/sh
cmd='cargo run'

$cmd << 'EOF'
timeout = 10

[hooks]
went_open = 'echo "$CHZZKD_ALIAS: $CHZZKD_LIVE_TITLE"'

[[channel]]
id = "c847a58a1599988f6154446c75366523"
alias = "dopa"

[[channel]]
id = "a7e175625fdea5a7d98428302b7aa57f"
alias = "chamcham"

[[channel]]
id = "6e06f5e1907f17eff543abd06cb62891"
alias = "nokduro"

[[channel]]
id = "9381e7d6816e6d915a44a13c0195b202"
alias = "lck"

[[channel]]
id = "0b33823ac81de48d5b78a38cdbc0ab94"
alias = "wolf"

[[channel]]
id = "42597020c1a79fb151bd9b9beaa9779b"
alias = "paka"

[[channel]]
id = "26ae7850ad5b6b09ca864d482dc7fa50"
alias = "qb"

[[channel]]
id = "c100f81959d1c17044be0541eed56f5b"
alias = "megajw"

[[channel]]
id = "b5ed5db484d04faf4d150aedd362f34b"
alias = "gg"

[[channel]]
id = "8b3e8e3a13201cff0836c69cfab62f45"
alias = "flame"

[[channel]]
id = "6cac96d5c9b7a9fd28903aa32fc61749"
alias = "hd"

[[channel]]
id = "bc2dbff369307b5c446224cce192c8b1"
alias = "goarosa"

[[channel]]
id = "732f6f16d20991243ec3f2d7afed8821"
alias = "0du"

[[channel]]
id = "96e44e40a448971244bfd9dd8c832505"
alias = "gn"
EOF
