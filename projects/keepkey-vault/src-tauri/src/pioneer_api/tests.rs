// Integration tests for Pioneer API client
use super::*;
use mockito::{Matcher, Mock};

#[tokio::test]
async fn test_get_portfolio_balances() {
    // Create a mock server
    let mut server = mockito::Server::new_async().await;
    let base_url = server.url();
    
    // Mock the portfolio balances endpoint
    let _m = server.mock("POST", "/api/v1/portfolio/balances")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[
            {
                "caip": "eip155:1/slip44:60",
                "ticker": "ETH",
                "balance": "1.23456789",
                "valueUsd": "3456.78",
                "priceUsd": "2800.00",
                "networkId": "eip155:1",
                "address": "0x1234567890123456789012345678901234567890",
                "type": "balance",
                "name": "Ethereum",
                "icon": "https://assets.coingecko.com/coins/images/279/large/ethereum.png",
                "precision": 18
            },
            {
                "caip": "bip122:000000000019d6689c085ae165831e93/slip44:0",
                "ticker": "BTC",
                "balance": "0.12345678",
                "valueUsd": "7654.32",
                "priceUsd": "62000.00",
                "networkId": "bip122:000000000019d6689c085ae165831e93",
                "address": "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh",
                "type": "balance",
                "name": "Bitcoin",
                "precision": 8
            }
        ]"#)
        .create();
    
    // Create client with mock URL
    let client = PioneerClient::with_base_url(base_url, None).unwrap();
    
    // Create test requests
    let requests = vec![
        PortfolioRequest {
            caip: "eip155:1/slip44:60".to_string(),
            pubkey: "xpub123...".to_string(),
        },
        PortfolioRequest {
            caip: "bip122:000000000019d6689c085ae165831e93/slip44:0".to_string(),
            pubkey: "xpub456...".to_string(),
        },
    ];
    
    // Call the API
    let balances = client.get_portfolio_balances(requests).await.unwrap();
    
    // Verify results
    assert_eq!(balances.len(), 2);
    assert_eq!(balances[0].ticker, "ETH");
    assert_eq!(balances[0].balance, "1.23456789");
    assert_eq!(balances[0].value_usd, "3456.78");
    assert_eq!(balances[1].ticker, "BTC");
    assert_eq!(balances[1].balance, "0.12345678");
}

#[tokio::test]
async fn test_get_staking_positions() {
    let mut server = mockito::Server::new_async().await;
    let base_url = server.url();
    
    let _m = server.mock("GET", "/api/v1/cosmos:cosmoshub-4/staking/cosmos1abc123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[
            {
                "validator": "cosmosvaloper1xyz",
                "amount": "100.0",
                "rewards": "5.5",
                "unbondingAmount": "10.0",
                "unbondingEnd": 1735689600
            }
        ]"#)
        .create();
    
    let client = PioneerClient::with_base_url(base_url, None).unwrap();
    
    let positions = client
        .get_staking_positions("cosmos:cosmoshub-4", "cosmos1abc123")
        .await
        .unwrap();
    
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].validator, "cosmosvaloper1xyz");
    assert_eq!(positions[0].amount, "100.0");
    assert_eq!(positions[0].rewards, "5.5");
}

#[tokio::test]
async fn test_get_charts_with_staking() {
    let mut server = mockito::Server::new_async().await;
    let base_url = server.url();
    
    let _m = server.mock("POST", "/api/v1/portfolio/charts")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[
            {
                "caip": "cosmos:cosmoshub-4/slip44:118",
                "ticker": "ATOM",
                "balance": "100.0",
                "valueUsd": "1234.56",
                "networkId": "cosmos:cosmoshub-4",
                "type": "delegation",
                "validator": "cosmosvaloper1xyz"
            },
            {
                "caip": "cosmos:cosmoshub-4/slip44:118",
                "ticker": "ATOM",
                "balance": "10.0",
                "valueUsd": "123.45",
                "networkId": "cosmos:cosmoshub-4",
                "type": "unbonding",
                "validator": "cosmosvaloper1xyz",
                "unbondingEnd": 1735689600
            }
        ]"#)
        .create();
    
    let client = PioneerClient::with_base_url(base_url, None).unwrap();
    
    let pubkeys = vec![
        PubkeyInfo {
            pubkey: "xpubCosmos...".to_string(),
            networks: vec!["cosmos:cosmoshub-4".to_string()],
            path: Some("m/44'/118'/0'".to_string()),
            address: Some("cosmos1abc123".to_string()),
        },
    ];
    
    let staking_data = client.get_charts(pubkeys).await.unwrap();
    
    assert_eq!(staking_data.len(), 2);
    assert_eq!(staking_data[0].balance_type, Some("delegation".to_string()));
    assert_eq!(staking_data[1].balance_type, Some("unbonding".to_string()));
}

#[tokio::test]
async fn test_build_portfolio() {
    let mut server = mockito::Server::new_async().await;
    let base_url = server.url();
    
    let _m = server.mock("POST", "/api/v1/portfolio/balances")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[
            {
                "caip": "eip155:1/slip44:60",
                "ticker": "ETH",
                "balance": "2.0",
                "valueUsd": "5600.00",
                "networkId": "eip155:1"
            },
            {
                "caip": "eip155:1/erc20:0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                "ticker": "USDC",
                "balance": "1000.0",
                "valueUsd": "1000.00",
                "networkId": "eip155:1"
            },
            {
                "caip": "bip122:000000000019d6689c085ae165831e93/slip44:0",
                "ticker": "BTC",
                "balance": "0.1",
                "valueUsd": "6200.00",
                "networkId": "bip122:000000000019d6689c085ae165831e93"
            }
        ]"#)
        .create();
    
    let client = PioneerClient::with_base_url(base_url, None).unwrap();
    
    let xpubs = vec!["xpub1", "xpub2"];
    let dashboard = client.build_portfolio(xpubs).await.unwrap();
    
    // Verify dashboard totals
    assert_eq!(dashboard.total_value_usd, 12800.0); // 5600 + 1000 + 6200
    
    // Verify network aggregation
    assert_eq!(dashboard.networks.len(), 2);
    assert_eq!(dashboard.networks[0].network_id, "eip155:1");
    assert_eq!(dashboard.networks[0].value_usd, 6600.0); // ETH + USDC
    assert_eq!(dashboard.networks[1].network_id, "bip122:000000000019d6689c085ae165831e93");
    assert_eq!(dashboard.networks[1].value_usd, 6200.0);
    
    // Verify asset aggregation
    assert_eq!(dashboard.assets.len(), 3);
    assert_eq!(dashboard.assets[0].ticker, "BTC");
    assert_eq!(dashboard.assets[0].value_usd, 6200.0);
}

#[tokio::test]
async fn test_api_error_handling() {
    let mut server = mockito::Server::new_async().await;
    let base_url = server.url();
    
    // Test unauthorized
    let _m = server.mock("POST", "/api/v1/portfolio/balances")
        .with_status(401)
        .with_body(r#"{"message": "Unauthorized", "error": {"name": "UnauthorizedError", "statusCode": 401}}"#)
        .create();
    
    let client = PioneerClient::with_base_url(base_url, None).unwrap();
    let result = client.get_portfolio_balances(vec![]).await;
    
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Unauthorized"));
}

#[tokio::test]
async fn test_service_unavailable_fallback() {
    let mut server = mockito::Server::new_async().await;
    let base_url = server.url();
    
    // Test service unavailable - should return empty vec instead of error
    let _m = server.mock("POST", "/api/v1/portfolio/balances")
        .with_status(503)
        .create();
    
    let client = PioneerClient::with_base_url(base_url, None).unwrap();
    let result = client.get_portfolio_balances(vec![]).await.unwrap();
    
    assert!(result.is_empty());
} 