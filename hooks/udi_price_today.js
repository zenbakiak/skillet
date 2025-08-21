// @name: UDI_PRICE_TODAY
// @min_args: 1
// @max_args: 2
// @description: Get UDI price with SQLite caching. UDI_PRICE_TODAY(banxico_token) or UDI_PRICE_TODAY(banxico_token, base_url). Uses skillet.db for caching.
// @example: UDI_PRICE_TODAY("your_token_here") returns latest UDI price

// Helper function to format dates as YYYY-MM-DD
function formatDate(date) {
    return date.getFullYear() + '-' +
           String(date.getMonth() + 1).padStart(2, '0') + '-' +
           String(date.getDate()).padStart(2, '0');
}

// Helper function to get date difference in days
function getDateDifferenceInDays(dateStr1, dateStr2) {
    const date1 = new Date(dateStr1);
    const date2 = new Date(dateStr2);
    const timeDiff = Math.abs(date2.getTime() - date1.getTime());
    return Math.ceil(timeDiff / (1000 * 3600 * 24));
}

// Database management functions
function ensureUdiTableExists(dbPath) {
    const tableCheckQuery = "SELECT name FROM sqlite_master WHERE type='table' AND name='udi_prices'";
    const tableCheckResult = sqliteQuery(dbPath, tableCheckQuery);
    const tableExists = JSON.parse(tableCheckResult).length > 0;

    if (!tableExists) {
        const createTableSQL = `
            CREATE TABLE udi_prices (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                fecha DATE NOT NULL UNIQUE,
                dato REAL NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
        `;
        const createResult = sqliteExec(dbPath, createTableSQL);
        if (!createResult.includes("OK")) {
            throw new Error("Error creating udi_prices table: " + createResult);
        }
    }
}

function getTodaysUdiPrice(dbPath, today) {
    const todayQuery = `SELECT dato FROM udi_prices WHERE fecha = '${today}' LIMIT 1`;
    const todayResult = JSON.parse(sqliteQuery(dbPath, todayQuery));
    
    return todayResult.length > 0 ? todayResult[0].dato : null;
}

function saveUdiPrice(dbPath, fecha, dato) {
    try {
        const insertSQL = `INSERT INTO udi_prices (fecha, dato) VALUES ('${fecha}', ${dato})`;
        sqliteExec(dbPath, insertSQL);
        return true;
    } catch (insertError) {
        // Insert failed, but we still have the data
        return false;
    }
}

// API request function
function fetchUdiFromBanxico(token, baseUrl, startDate, endDate) {
    try {
        const apiUrl = `${baseUrl}/SP68257/datos/${startDate}/${endDate}?token=${token}`;
        const response = httpGet(apiUrl);
        const data = JSON.parse(response);

        // Extract the data following the API structure
        if (data && data.bmx && data.bmx.series && data.bmx.series.length > 0) {
            const series = data.bmx.series[0];
            if (series.datos && series.datos.length > 0) {
                // Get the most recent data point
                const latestData = series.datos[series.datos.length - 1];
                return {
                    fecha: latestData.fecha,
                    dato: parseFloat(latestData.dato)
                };
            }
        }
        return null;
    } catch (apiError) {
        return null;
    }
}

// Fallback logic function
function getRecentUdiPrice(dbPath, today) {
    const fallbackQuery = `
        SELECT fecha, dato FROM udi_prices
        ORDER BY fecha DESC
        LIMIT 1
    `;
    const fallbackResult = JSON.parse(sqliteQuery(dbPath, fallbackQuery));

    if (fallbackResult.length > 0) {
        const lastRecord = fallbackResult[0];
        const daysDiff = getDateDifferenceInDays(lastRecord.fecha, today);

        if (daysDiff <= 2) {
            return lastRecord.dato;
        }
    }
    return null;
}

// Main execution function
function execute(args) {
    // Validate arguments
    if (args.length < 1) {
        return "Error: banxico_token parameter is required";
    }

    const token = args[0];
    const baseUrl = args.length > 1 ? args[1] : "https://www.banxico.org.mx/SieAPIRest/service/v1/series";
    const dbPath = "skillet.db";
    const today = formatDate(new Date());

    try {
        // Step 1: Ensure database table exists
        ensureUdiTableExists(dbPath);

        // Step 2: Check if we have today's price cached
        const cachedPrice = getTodaysUdiPrice(dbPath, today);
        if (cachedPrice !== null) {
            return cachedPrice;
        }

        // Step 3: Try to get fresh data from API
        const yesterday = new Date();
        yesterday.setDate(yesterday.getDate() - 1);
        const startDate = formatDate(yesterday);
        
        const apiData = fetchUdiFromBanxico(token, baseUrl, startDate, today);
        
        if (apiData) {
            // Save to database and return the value
            saveUdiPrice(dbPath, apiData.fecha, apiData.dato);
            return apiData.dato;
        }

        // Step 4: API failed, try to get recent cached data
        const recentPrice = getRecentUdiPrice(dbPath, today);
        if (recentPrice !== null) {
            return recentPrice;
        }

        // Step 5: No suitable data available
        return "Error: Unable to get UDI price from API and no recent cached data available";

    } catch (e) {
        return "Error accessing database: " + e.message;
    }
}