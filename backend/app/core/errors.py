"""Global exception handlers: normalize errors into JSON detail payloads."""
import logging

import sqlite3
from fastapi import FastAPI, HTTPException, Request
from fastapi.responses import JSONResponse

log = logging.getLogger("app")


def register_exception_handlers(app: FastAPI) -> None:
    @app.exception_handler(HTTPException)
    async def http_exc(request: Request, exc: HTTPException):
        return JSONResponse(
            status_code=exc.status_code,
            content={"detail": exc.detail},
            headers=exc.headers,
        )

    @app.exception_handler(sqlite3.IntegrityError)
    async def integrity_exc(request: Request, exc: sqlite3.IntegrityError):
        return JSONResponse(status_code=409, content={"detail": "resource conflict"})

    @app.exception_handler(Exception)
    async def generic_exc(request: Request, exc: Exception):
        log.exception("unhandled error on %s %s", request.method, request.url.path)
        return JSONResponse(status_code=500, content={"detail": "internal server error"})
