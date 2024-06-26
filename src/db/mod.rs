pub mod models;

use self::models::{
    dex_trade::DatabaseDexTrade,
    erc1155_transfer::DatabaseERC1155Transfer,
    erc20_transfer::DatabaseERC20Transfer,
    erc721_transfer::DatabaseERC721Transfer,
};
use crate::{
    chains::Chain,
    explorer::models::{ChartTransactionResponse, InfoForAverageBlock},
};
use clickhouse::{Client, Row};
use futures::future::join_all;
use log::{error, info};
use models::{
    block::DatabaseBlock, contract::DatabaseContract,
    infoforsync::DatabaseInfoForSync, log::DatabaseLog,
    trace::DatabaseTrace, transaction::DatabaseTransaction,
    withdrawal::DatabaseWithdrawal,
};
use serde::Serialize;
use std::collections::HashSet;

pub struct BlockFetchedData {
    pub blocks: Vec<DatabaseBlock>,
    pub contracts: Vec<DatabaseContract>,
    pub logs: Vec<DatabaseLog>,
    pub traces: Vec<DatabaseTrace>,
    pub transactions: Vec<DatabaseTransaction>,
    pub withdrawals: Vec<DatabaseWithdrawal>,
    pub erc20_transfers: Vec<DatabaseERC20Transfer>,
    pub erc721_transfers: Vec<DatabaseERC721Transfer>,
    pub erc1155_transfers: Vec<DatabaseERC1155Transfer>,
    pub dex_trades: Vec<DatabaseDexTrade>,
}

#[derive(Clone)]
pub struct Database {
    pub chain: Chain,
    pub db: Client,
}

pub enum DatabaseTables {
    Blocks,
    Contracts,
    Logs,
    Traces,
    Transactions,
    Withdrawals,
    Erc20Transfers,
    Erc721Transfers,
    Erc1155Transfers,
    DexTrades,
    InfoForSync,
}

impl DatabaseTables {
    pub fn as_str(&self) -> &'static str {
        match self {
            DatabaseTables::Blocks => "blocks",
            DatabaseTables::Contracts => "contracts",
            DatabaseTables::Logs => "logs",
            DatabaseTables::Traces => "traces",
            DatabaseTables::Transactions => "transactions",
            DatabaseTables::Withdrawals => "withdrawals",
            DatabaseTables::Erc20Transfers => "erc20_transfers",
            DatabaseTables::Erc721Transfers => "erc721_transfers",
            DatabaseTables::Erc1155Transfers => "erc1155_transfers",
            DatabaseTables::DexTrades => "dex_trades",
            DatabaseTables::InfoForSync => "infoforsync",
        }
    }
}

impl Database {
    pub async fn new(
        db_host: String,
        db_username: String,
        db_password: String,
        db_name: String,
        chain: Chain,
    ) -> Self {
        let db = Client::default()
            .with_url(db_host)
            .with_user(db_username)
            .with_password(db_password)
            .with_database(db_name);

        Self { chain, db }
    }

    pub async fn get_indexed_blocks(&self) -> HashSet<u32> {
        let query = format!(
            "SELECT number FROM blocks WHERE chain = {} AND is_uncle = false",
            self.chain.id
        );

        let tokens = match self.db.query(&query).fetch_all::<u32>().await {
            Ok(tokens) => tokens,
            Err(_) => Vec::new(),
        };

        let blocks: HashSet<u32> = HashSet::from_iter(tokens.into_iter());

        blocks
    }

    pub async fn get_blocks(
        &self,
        skip_count: u32,
        limit: u32,
    ) -> Vec<DatabaseBlock> {
        // Build SQL query string.
        let query = format!(
            "SELECT * FROM blocks WHERE chain = {} AND is_uncle = false ORDER BY number DESC LIMIT {} OFFSET {}",
            self.chain.id, limit, skip_count
        );

        // Log the query string for debugging purposes.
        info!("{}", query);

        // Execute the query and return the result if successful. Otherwise, log the error and return an empty vector.
        match self.db.query(&query).fetch_all::<DatabaseBlock>().await {
            Ok(tokens) => tokens,
            Err(e) => {
                error!("Error fetching blocks from the database: {}", e);
                Vec::new()
            }
        }
    }

    pub async fn get_block_by_id(&self, number: u64) -> DatabaseBlock {
        let query = format!(
            "SELECT * FROM blocks WHERE chain = {} AND is_uncle = false AND number = {}",
            self.chain.id, number
        );

        match self.db.query(&query).fetch_one().await {
            Ok(token) => token,
            Err(e) => {
                error!("Error fetching block by id: {}", e);
                DatabaseBlock::new()
            }
        }
    }

    pub async fn get_transactions(
        &self,
        skip_count: u32,
        limit: u32,
    ) -> Vec<DatabaseTransaction> {
        // Build SQL query string.
        let query = format!(
            "SELECT * FROM transactions WHERE chain = {} ORDER BY timestamp DESC LIMIT {} OFFSET {}",
            self.chain.id, limit, skip_count
        );

        // Log the query string for debugging purposes.
        info!("{}", query);

        // Execute the query and return the result if successful. Otherwise, log the error and return an empty vector.
        match self
            .db
            .query(&query)
            .fetch_all::<DatabaseTransaction>()
            .await
        {
            Ok(tokens) => tokens,
            Err(e) => {
                error!(
                    "Error fetching transaction from the database: {}",
                    e
                );
                Vec::new()
            }
        }
    }

    pub async fn get_transaction_by_id(
        &self,
        hash: String,
    ) -> DatabaseTransaction {
        let query = format!(
            "SELECT * FROM transactions WHERE chain = {} AND hash = '{}'",
            self.chain.id, hash
        );
        info!("{}", query);
        match self.db.query(&query).fetch_one().await {
            Ok(token) => token,
            Err(e) => {
                error!("Error fetching block by id: {}", e);
                DatabaseTransaction::new()
            }
        }
    }

    pub async fn get_info_for_average_block(&self) -> InfoForAverageBlock {
        let query = format!(
            "SELECT Min(timestamp) as start_time, Max(timestamp) as end_time, Min(number) as start_number, Max(number) as end_number FROM blocks WHERE number IN (SELECT number FROM blocks WHERE chain = {} ORDER BY number DESC LIMIT 50)",
            self.chain.id
        );
        match self.db.query(&query).fetch_one().await {
            Ok(token) => token,
            Err(e) => {
                error!("Error fetching timestamp and number: {}", e);
                InfoForAverageBlock::new()
            }
        }
    }

    pub async fn get_chart_transaction_data(
        &self,
    ) -> Vec<ChartTransactionResponse> {
        info!("get chart transaction data");
        let query = format!(
            "SELECT toString(DATE(timestamp)) as tx_date, toUInt32(COUNT(DATE(timestamp))) as tx_count FROM transactions WHERE chain = {} GROUP BY DATE(timestamp) ORDER BY DATE(timestamp) DESC LIMIT 30",
            self.chain.id
        );

        match self
            .db
            .query(&query)
            .fetch_all::<ChartTransactionResponse>()
            .await
        {
            Ok(token) => token,
            Err(e) => {
                error!("Error fetching timestamp and number: {}", e);
                Vec::new()
            }
        }
    }

    pub async fn get_info_for_sync(&self) -> Vec<DatabaseInfoForSync> {
        info!("get info for sync");
        let query =
            "SELECT * FROM infoforsync ORDER BY timestamp DESC LIMIT 2"
                .to_string();

        match self
            .db
            .query(&query)
            .fetch_all::<DatabaseInfoForSync>()
            .await
        {
            Ok(token) => token,
            Err(e) => {
                error!("Error fetching timestamp and number: {}", e);
                Vec::new()
            }
        }
    }

    pub async fn store_data(&self, data: &BlockFetchedData) {
        let mut stores = vec![];
        if !data.contracts.is_empty() {
            let work = tokio::spawn({
                let contracts = data.contracts.clone();
                let db = self.clone();
                async move {
                    db.store_items(
                        &contracts,
                        DatabaseTables::Contracts.as_str(),
                    )
                    .await
                }
            });
            stores.push(work);
        }

        if !data.logs.is_empty() {
            let work = tokio::spawn({
                let logs = data.logs.clone();
                let db = self.clone();
                async move {
                    db.store_items(&logs, DatabaseTables::Logs.as_str())
                        .await
                }
            });

            stores.push(work);
        }

        if !data.traces.is_empty() {
            let work = tokio::spawn({
                let traces = data.traces.clone();
                let db = self.clone();
                async move {
                    db.store_items(
                        &traces,
                        DatabaseTables::Traces.as_str(),
                    )
                    .await
                }
            });

            stores.push(work);
        }

        if !data.transactions.is_empty() {
            let work = tokio::spawn({
                let transactions = data.transactions.clone();
                let db = self.clone();
                async move {
                    db.store_items(
                        &transactions,
                        DatabaseTables::Transactions.as_str(),
                    )
                    .await
                }
            });

            stores.push(work);
        }

        if !data.withdrawals.is_empty() {
            let work = tokio::spawn({
                let withdrawals: Vec<DatabaseWithdrawal> =
                    data.withdrawals.clone();
                let db = self.clone();
                async move {
                    db.store_items(
                        &withdrawals,
                        DatabaseTables::Withdrawals.as_str(),
                    )
                    .await
                }
            });

            stores.push(work);
        }

        if !data.erc20_transfers.is_empty() {
            let work = tokio::spawn({
                let transfers: Vec<DatabaseERC20Transfer> =
                    data.erc20_transfers.clone();
                let db = self.clone();
                async move {
                    db.store_items(
                        &transfers,
                        DatabaseTables::Erc20Transfers.as_str(),
                    )
                    .await
                }
            });

            stores.push(work);
        }

        if !data.erc721_transfers.is_empty() {
            let work = tokio::spawn({
                let transfers: Vec<DatabaseERC721Transfer> =
                    data.erc721_transfers.clone();
                let db = self.clone();
                async move {
                    db.store_items(
                        &transfers,
                        DatabaseTables::Erc721Transfers.as_str(),
                    )
                    .await
                }
            });

            stores.push(work);
        }

        if !data.erc1155_transfers.is_empty() {
            let work = tokio::spawn({
                let transfers: Vec<DatabaseERC1155Transfer> =
                    data.erc1155_transfers.clone();
                let db = self.clone();
                async move {
                    db.store_items(
                        &transfers,
                        DatabaseTables::Erc1155Transfers.as_str(),
                    )
                    .await
                }
            });

            stores.push(work);
        }

        if !data.dex_trades.is_empty() {
            let work = tokio::spawn({
                let trades: Vec<DatabaseDexTrade> =
                    data.dex_trades.clone();
                let db = self.clone();
                async move {
                    db.store_items(
                        &trades,
                        DatabaseTables::DexTrades.as_str(),
                    )
                    .await
                }
            });

            stores.push(work);
        }

        let res = join_all(stores).await;

        let errored: Vec<_> =
            res.iter().filter(|res| res.is_err()).collect();

        if !errored.is_empty() {
            panic!("failed to store all chain primitive elements")
        }

        if !data.blocks.is_empty() {
            self.store_items(
                &data.blocks,
                DatabaseTables::Blocks.as_str(),
            )
            .await;
        }

        info!(
            "Inserted: contracts ({}) logs ({}) traces ({}) transactions ({}) withdrawals ({}) erc20 ({}) erc721 ({}) erc1155 ({}) dex_trades ({}) in ({}) blocks.",
            data.contracts.len(),
            data.logs.len(),
            data.traces.len(),
            data.transactions.len(),
            data.withdrawals.len(),
            data.erc20_transfers.len(),
            data.erc721_transfers.len(),
            data.erc1155_transfers.len(),
            data.dex_trades.len(),
            data.blocks.len()
        );
    }

    pub async fn store_items<T>(&self, items: &Vec<T>, table: &str)
    where
        T: Row + Serialize,
    {
        let mut inserter = self.db.inserter(table).unwrap();

        for item in items {
            inserter.write(item).await.unwrap();
        }

        match inserter.end().await {
            Ok(_) => (),
            Err(err) => {
                error!("{}", err);
                panic!("Unable to store {} into database", table)
            }
        }
    }
}
