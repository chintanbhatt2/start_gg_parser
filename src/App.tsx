import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import "./App.css";

type CsvPreview = {
  headers: string[];
  rows: string[][];
};

type DownloadProgressEvent = {
  tournamentSlug: string;
  page: number;
  message: string;
  done: boolean;
};

const VIEW_MODE_STORAGE_KEY = "startgg.previewMode";
const AUTH_TOKEN_STORAGE_KEY = "startgg.authToken";
const RECENT_SLUGS_STORAGE_KEY = "startgg.recentSlugs";
const MAX_RECENT_SLUGS = 12;

function getStoredPreviewMode(): "form" | "table" {
  const storedMode = localStorage.getItem(VIEW_MODE_STORAGE_KEY);
  return storedMode === "form" ? "form" : "table";
}

function getStoredAuthToken(): string {
  return localStorage.getItem(AUTH_TOKEN_STORAGE_KEY) ?? "";
}

function getStoredRecentSlugs(): string[] {
  const rawValue = localStorage.getItem(RECENT_SLUGS_STORAGE_KEY);
  if (!rawValue) {
    return [];
  }

  try {
    const parsed = JSON.parse(rawValue);
    if (!Array.isArray(parsed)) {
      return [];
    }

    return parsed
      .filter((item): item is string => typeof item === "string")
      .map((item) => item.trim())
      .filter((item) => item.length > 0)
      .slice(0, MAX_RECENT_SLUGS);
  } catch {
    return [];
  }
}

function parseCsvLine(line: string): string[] {
  const cells: string[] = [];
  let current = "";
  let inQuotes = false;

  for (let i = 0; i < line.length; i += 1) {
    const ch = line[i];
    if (ch === '"') {
      if (inQuotes && line[i + 1] === '"') {
        current += '"';
        i += 1;
      } else {
        inQuotes = !inQuotes;
      }
      continue;
    }

    if (ch === "," && !inQuotes) {
      cells.push(current);
      current = "";
      continue;
    }

    current += ch;
  }

  cells.push(current);
  return cells;
}

function parseCsv(csvText: string): CsvPreview {
  const lines = csvText
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.length > 0);

  if (lines.length === 0) {
    return { headers: [], rows: [] };
  }

  const headers = parseCsvLine(lines[0]);
  const rows = lines.slice(1).map((line) => parseCsvLine(line));
  return { headers, rows };
}

function App() {
  const [statusMsg, setStatusMsg] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [progressMsg, setProgressMsg] = useState("");
  const [name, setName] = useState("");
  const [token, setToken] = useState<string>(() => getStoredAuthToken());
  const [csvData, setCsvData] = useState("");
  const [csvPreview, setCsvPreview] = useState<CsvPreview>({ headers: [], rows: [] });
  const [previewMode, setPreviewMode] = useState<"form" | "table">(() => getStoredPreviewMode());
  const [recentSlugs, setRecentSlugs] = useState<string[]>(() => getStoredRecentSlugs());

  useEffect(() => {
    localStorage.setItem(VIEW_MODE_STORAGE_KEY, previewMode);
  }, [previewMode]);

  useEffect(() => {
    localStorage.setItem(AUTH_TOKEN_STORAGE_KEY, token);
  }, [token]);

  useEffect(() => {
    localStorage.setItem(RECENT_SLUGS_STORAGE_KEY, JSON.stringify(recentSlugs));
  }, [recentSlugs]);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    (async () => {
      unlisten = await listen<DownloadProgressEvent>("download-progress", (event) => {
        setProgressMsg(event.payload.message);
      });
    })();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const hasCsvPreview = useMemo(() => csvPreview.headers.length > 0 && csvPreview.rows.length > 0, [csvPreview]);

  function clearCsvPreview() {
    setCsvData("");
    setCsvPreview({ headers: [], rows: [] });
  }

  function getTournamentNames(): string[] {
    return name
      .split(",")
      .map((value) => value.trim())
      .filter((value) => value.length > 0);
  }

  function rememberTournamentSlugs(slugs: string[]) {
    const normalized = slugs.map((slug) => slug.trim()).filter((slug) => slug.length > 0);
    if (normalized.length === 0) {
      return;
    }

    setRecentSlugs((previous) => {
      const merged = [...normalized, ...previous];
      const deduped = merged.filter((slug, index) => merged.indexOf(slug) === index);
      return deduped.slice(0, MAX_RECENT_SLUGS);
    });
  }

  async function fetchTournamentCsv(): Promise<string | null> {
    const tournamentNames = getTournamentNames();

    if (tournamentNames.length === 0) {
      clearCsvPreview();
      setStatusMsg("Enter at least one tournament slug.");
      return null;
    }

    setProgressMsg("Preparing download...");
    setIsLoading(true);

    let responseCsv: string;
    try {
      responseCsv = await invoke<string>("get_tournament_rows_csv", {
        tournamentNames,
        authToken: token,
      });
    } catch (error) {
      clearCsvPreview();
      const errorMessage = error instanceof Error ? error.message : String(error);
      setStatusMsg(errorMessage);
      return null;
    } finally {
      setIsLoading(false);
      setProgressMsg("");
    }

    const parsed = parseCsv(responseCsv);
    if (parsed.headers.length === 0 || parsed.rows.length === 0) {
      clearCsvPreview();
      setStatusMsg("Tournament download failed: empty CSV response.");
      return null;
    }

    setCsvData(responseCsv);
    setCsvPreview(parsed);
    rememberTournamentSlugs(tournamentNames);
    setStatusMsg(`Loaded ${parsed.rows.length} CSV rows.`);
    return responseCsv;
  }

  async function handleDownloadTournament() {
    await fetchTournamentCsv();
  }

  async function exportCsv() {
    const currentCsv = csvData || (await fetchTournamentCsv());
    if (!currentCsv) {
      return;
    }

    const filePath = await save({
      defaultPath: "tournament_rows.csv",
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });

    if (!filePath) {
      setStatusMsg("CSV export canceled.");
      return;
    }

    try {
      const saveResult = await invoke<string>("save_text_file", {
        path: filePath,
        contents: currentCsv,
      });
      setStatusMsg(saveResult);
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      setStatusMsg(errorMessage);
    }
  }

  return (
    <main className="container">
      Tournament Slugs should be formatted like "tournament/tournament-name" and multiple slugs can be separated by commas.

      <form
        className="row"
        onSubmit={(e) => {
          e.preventDefault();
          handleDownloadTournament();
        }}
      >

        <input
          id="greet-input"
          value={name}
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="Enter tournament slug(s)"
          disabled={isLoading}
        />
        <input
          id="token-input"
          value={token}
          onChange={(e) => setToken(e.currentTarget.value)}
          placeholder="Enter your auth token..."
          disabled={isLoading}
        />
        <button type="submit" disabled={isLoading}>Download Tournament</button>
        <button type="button" onClick={exportCsv} disabled={isLoading}>Export CSV</button>
      </form>

      {recentSlugs.length > 0 && (
        <section className="recent-slugs" aria-label="Recent tournament slugs">
          <div className="recent-slugs-header">
            <h3>Recent Slugs</h3>
            <button type="button" onClick={() => setRecentSlugs([])}>Clear History</button>
          </div>
          <div className="recent-slugs-list">
            {recentSlugs.map((slug) => (
              <button key={slug} type="button" className="recent-slug-chip" onClick={() => setName(slug)}>
                {slug}
              </button>
            ))}
          </div>
        </section>
      )}

      {isLoading && (
        <div className="loading-indicator" role="status" aria-live="polite">
          <span className="spinner" aria-hidden="true" />
          <span>{progressMsg || "Downloading tournament data..."}</span>
        </div>
      )}

      <p>{statusMsg}</p>

      {hasCsvPreview && (
        <section className="csv-preview">
          <div className="csv-preview-header">
            <h2>CSV Components</h2>
            <div className="preview-toggle" role="group" aria-label="CSV preview mode">
              <button
                type="button"
                className={previewMode === "table" ? "active" : ""}
                onClick={() => setPreviewMode("table")}
              >
                Table View
              </button>
              <button
                type="button"
                className={previewMode === "form" ? "active" : ""}
                onClick={() => setPreviewMode("form")}
              >
                Form View
              </button>
            </div>
          </div>

          {previewMode === "form" ? (
            csvPreview.rows.map((row, rowIndex) => (
              <fieldset key={`row-${rowIndex}`} className="csv-row-card">
                <legend>Row {rowIndex + 1}</legend>
                {csvPreview.headers.map((header, colIndex) => (
                  <label key={`${header}-${rowIndex}-${colIndex}`} className="csv-field">
                    <span>{header}</span>
                    <input value={row[colIndex] ?? ""} readOnly />
                  </label>
                ))}
              </fieldset>
            ))
          ) : (
            <div className="csv-table-wrap">
              <table className="csv-table">
                <thead>
                  <tr>
                    {csvPreview.headers.map((header) => (
                      <th key={header}>{header}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {csvPreview.rows.map((row, rowIndex) => (
                    <tr key={`table-row-${rowIndex}`}>
                      {csvPreview.headers.map((header, colIndex) => (
                        <td key={`${header}-table-${rowIndex}-${colIndex}`}>{row[colIndex] ?? ""}</td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>
      )}
    </main>
  );
}

export default App;
