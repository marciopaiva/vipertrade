import { NextResponse } from "next/server";

type ControlKind = "kill-switch" | "executor" | "risk-limits";

const DEFAULT_BASE_URLS = [
  process.env.BACKEND_API_URL,
  "http://host.containers.internal:8080/api/v1",
  "http://host.docker.internal:8080/api/v1",
  "http://api:8080/api/v1",
  "http://vipertrade-api:8080/api/v1",
  process.env.NEXT_PUBLIC_API_URL,
  "http://localhost:8080/api/v1",
  "http://127.0.0.1:8080/api/v1",
].filter(Boolean) as string[];

function uniqueBaseUrls(baseUrls: string[]): string[] {
  return Array.from(new Set(baseUrls.map((v) => v.replace(/\/+$/, ""))));
}

function resolvePath(kind: ControlKind): string {
  switch (kind) {
    case "kill-switch":
      return "/control/kill-switch";
    case "executor":
      return "/control/executor";
    case "risk-limits":
      return "/control/risk-limits";
  }
}

export async function POST(req: Request) {
  const body = (await req.json()) as {
    kind?: ControlKind;
    payload?: Record<string, unknown>;
    operatorToken?: string;
    operatorId?: string;
  };

  if (!body.kind || !["kill-switch", "executor", "risk-limits"].includes(body.kind)) {
    return NextResponse.json({ error: "invalid_kind", message: "invalid control command" }, { status: 400 });
  }

  const token = body.operatorToken?.trim() || process.env.OPERATOR_API_TOKEN;
  if (!token) {
    return NextResponse.json(
      { error: "missing_token", message: "operator token is required" },
      { status: 401 },
    );
  }

  const operatorId = body.operatorId?.trim() || "web-operator";
  const baseUrls = uniqueBaseUrls(DEFAULT_BASE_URLS);
  const path = resolvePath(body.kind);
  const errors: Array<{ baseUrl: string; message: string }> = [];

  for (const baseUrl of baseUrls) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 5000);

    try {
      const response = await fetch(`${baseUrl}${path}`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          "x-operator-token": token,
          "x-operator-id": operatorId,
        },
        body: JSON.stringify(body.payload ?? {}),
        signal: controller.signal,
      });

      const raw = await response.text();
      const parsed = raw ? JSON.parse(raw) : null;

      if (!response.ok) {
        errors.push({
          baseUrl,
          message: `http=${response.status} body=${raw || "<empty>"}`,
        });
        continue;
      }

      return NextResponse.json(
        {
          ok: true,
          source: baseUrl,
          kind: body.kind,
          result: parsed,
        },
        { status: 200 },
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      errors.push({ baseUrl, message });
    } finally {
      clearTimeout(timeout);
    }
  }

  return NextResponse.json(
    {
      ok: false,
      error: "control_unavailable",
      message: "could not send control command to backend",
      details: errors,
    },
    { status: 502 },
  );
}
