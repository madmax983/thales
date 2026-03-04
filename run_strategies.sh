#!/bin/bash
STRATEGIES=(
    BollingerBands
    EmaCrossover
    RsiMeanReversion
    Macd
    Supertrend
    DonchianBreakout
    ParabolicSar
    KeltnerChannelBreakout
    StochasticOscillator
    AdxMomentum
    IchimokuCloud
    CciMomentum
    LinearRegressionTrend
    ObvTrendFollowing
    MoneyFlowIndex
    ConnorsRsiMeanReversion
    AwesomeOscillator
    WilliamsR
    VwmaCrossover
    VwapReversion
    VortexBreakout
    ZScoreMeanReversion
    ChaikinMoneyFlow
    ElderRay
    AroonOscillator
    RocMomentum
)

for ASSET in btc eth spy; do
    echo "Running strategies for $ASSET"
    for STRAT in "${STRATEGIES[@]}"; do
        env SIMULATION=true cargo run -p thales-cli -- generate-signals --input ${ASSET}_data.json --strategy ${STRAT} --analysis ${ASSET}_analysis.json > ${ASSET}_${STRAT}.json 2> /dev/null
        # Check if output is empty signal []
        if grep -q "\"data\":\[\]" ${ASSET}_${STRAT}.json; then
            rm ${ASSET}_${STRAT}.json
        else
            echo "$STRAT generated a signal for $ASSET"
        fi
    done
done
