# Paginating API results

List endpoints return their results one page at a time, with an opaque
cursor that leads to the next page.

## Requesting a page

```http
GET /v2/invoices?page_size=50&cursor=b3JkZXItMTIw
```

`page_size` defaults to 25 and cannot exceed 200; a larger value is refused
with `400 invalid_request`. Leave out `cursor` to get the first page.

## Reading the response

```json
{
  "items": [],
  "next_cursor": "b3JkZXItMTcw",
  "has_more": true
}
```

Pass `next_cursor` as `cursor` to fetch the next page. When `has_more` is
false, `next_cursor` is null and the list is complete.

## Cursor lifetime

A cursor stays valid for 10 minutes. After that, the API answers `410 Gone`
with the error code `cursor_expired`, and the listing must start again from
the first page.

## Sorting and consistency

Items are sorted by creation time, then by id, so that two pages never
overlap. An item created while you page through a list appears only if it
sorts after the current cursor.
