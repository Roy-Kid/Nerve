import * as http from "node:http";
import { HUB_HOST, HUB_PORT } from "./endpoint";

export class HubHttpError extends Error {
  constructor(
    message: string,
    readonly status?: number,
  ) {
    super(message);
    this.name = "HubHttpError";
  }
}

export function getText(
  path: string,
  timeoutMs: number,
): Promise<string> {
  return new Promise((resolve, reject) => {
    const request = http.get(
      {
        host: HUB_HOST,
        port: HUB_PORT,
        path,
        timeout: timeoutMs,
        headers: { Accept: "application/json, text/event-stream, */*" },
      },
      (response) => {
        const status = response.statusCode ?? 0;
        if (status < 200 || status >= 300) {
          response.resume();
          reject(new HubHttpError(`hub answered HTTP ${status}`, status));
          return;
        }
        const chunks: Buffer[] = [];
        response.on("data", (chunk: Buffer) => {
          chunks.push(chunk);
        });
        response.on("end", () => {
          resolve(Buffer.concat(chunks).toString("utf8"));
        });
        response.on("error", reject);
      },
    );
    request.on("timeout", () => {
      request.destroy(new HubHttpError("hub request timed out"));
    });
    request.on("error", reject);
  });
}

export function postText(
  path: string,
  timeoutMs: number,
): Promise<string> {
  return new Promise((resolve, reject) => {
    const request = http.request(
      {
        host: HUB_HOST,
        port: HUB_PORT,
        path,
        method: "POST",
        timeout: timeoutMs,
        headers: { Accept: "application/json, */*", "Content-Length": 0 },
      },
      (response) => {
        const status = response.statusCode ?? 0;
        if (status < 200 || status >= 300) {
          response.resume();
          reject(new HubHttpError(`hub answered HTTP ${status}`, status));
          return;
        }
        const chunks: Buffer[] = [];
        response.on("data", (chunk: Buffer) => {
          chunks.push(chunk);
        });
        response.on("end", () => {
          resolve(Buffer.concat(chunks).toString("utf8"));
        });
        response.on("error", reject);
      },
    );
    request.on("timeout", () => {
      request.destroy(new HubHttpError("hub request timed out"));
    });
    request.on("error", reject);
    request.end();
  });
}

export function getStream(
  path: string,
  onData: (chunk: string) => void,
  onEnd: () => void,
): http.ClientRequest {
  const request = http.get(
    {
      host: HUB_HOST,
      port: HUB_PORT,
      path,
      headers: { Accept: "text/event-stream" },
    },
    (response) => {
      const status = response.statusCode ?? 0;
      if (status < 200 || status >= 300) {
        response.resume();
        onEnd();
        return;
      }
      response.setEncoding("utf8");
      response.on("data", (chunk: string) => {
        onData(chunk);
      });
      response.on("end", onEnd);
      response.on("error", onEnd);
    },
  );
  request.on("error", onEnd);
  return request;
}

export function sendJson(path: string, method: string, value: unknown): Promise<void> {
  return new Promise((resolve, reject) => {
    const body = JSON.stringify(value);
    const request = http.request({host:HUB_HOST,port:HUB_PORT,path,method,timeout:5000,
      headers:{"Content-Type":"application/json","Content-Length":Buffer.byteLength(body)}}, response => {
      response.resume(); response.on("end", () => (response.statusCode ?? 500) < 300 ? resolve() : reject(new Error(`hub HTTP ${response.statusCode}`)));
    });
    request.on("error",reject); request.on("timeout",()=>request.destroy(new Error("hub timeout"))); request.end(body);
  });
}
