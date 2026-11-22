import { handleCollectScriptProxy, handleEventProxy } from "./proxy.ts";
import type {
  CollectProxyOptions,
  EventProxyOptions,
  ExecutionContextLike,
  RotameterEnv,
} from "./types.ts";

export interface AstroLocalsLike {
  runtime?: {
    env?: RotameterEnv;
    ctx?: ExecutionContextLike;
  };
  cfContext?: ExecutionContextLike;
}

export interface AstroRouteContext {
  request: Request;
  locals: AstroLocalsLike;
}

export interface AstroHandlerOptions {
  getEnv?: (locals: AstroLocalsLike) => RotameterEnv;
  getCtx?: (locals: AstroLocalsLike) => ExecutionContextLike | undefined;
}

function resolveEnv(locals: AstroLocalsLike, getEnv?: (l: AstroLocalsLike) => RotameterEnv): RotameterEnv {
  return getEnv ? getEnv(locals) : (locals.runtime?.env ?? {});
}

function resolveCtx(
  locals: AstroLocalsLike,
  getCtx?: (l: AstroLocalsLike) => ExecutionContextLike | undefined,
): ExecutionContextLike | undefined {
  return getCtx ? getCtx(locals) : (locals.cfContext ?? locals.runtime?.ctx);
}

export function createCollectScriptRoute(
  opts: CollectProxyOptions & AstroHandlerOptions = {},
): (ctx: AstroRouteContext) => Promise<Response> {
  return async ({ request, locals }) =>
    handleCollectScriptProxy(request, resolveEnv(locals, opts.getEnv), opts);
}

export function createEventProxyRoute(
  opts: EventProxyOptions & AstroHandlerOptions = {},
): (ctx: AstroRouteContext) => Promise<Response> {
  return async ({ request, locals }) =>
    handleEventProxy(
      request,
      resolveEnv(locals, opts.getEnv),
      resolveCtx(locals, opts.getCtx),
      opts,
    );
}
