forge flatten src/ExchangeHub.sol > flattened/ExchangeHub_Full_flattened.sol
forge flatten ./src/ExchangeChannel.sol -o flattened/ExchangeChannelFlattened.sol
forge flatten ./src/oracle/FunctionsConsumer_Lighthouse.sol -o flattened/FunctionsConsumer_Lighthouse_Flattened.sol
forge flatten ./src/oracle/FunctionsConsumer_Walrus.sol -o flattened/FunctionsConsumer_Walrus_Flattened.sol
forge flatten ./src/oracle/OracleProxy.sol -o flattened/OracleProxy.sol