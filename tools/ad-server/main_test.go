package main

import (
	"bytes"
	"fmt"
	"image/jpeg"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func get(t *testing.T, server *httptest.Server, path string) (int, []byte) {
	t.Helper()
	response, err := http.Get(server.URL + path)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	body, err := io.ReadAll(response.Body)
	if err != nil {
		t.Fatal(err)
	}
	return response.StatusCode, body
}

func TestManifestListsEveryServedAd(t *testing.T) {
	ads, err := generate()
	if err != nil {
		t.Fatal(err)
	}
	server := httptest.NewServer(handler(ads))
	defer server.Close()

	if status, body := get(t, server, apiPrefix+"/ping"); status != 200 || string(body) != "skate2-ads 1\n" {
		t.Fatalf("ping = %d %q", status, body)
	}
	status, body := get(t, server, apiPrefix+"/manifest.txt")
	lines := strings.Split(strings.TrimSpace(string(body)), "\n")
	if status != 200 || lines[0] != "skate2-ads 1" || len(lines) != 1+len(ads) {
		t.Fatalf("manifest = %d with %d lines", status, len(lines))
	}
	for _, line := range lines[1:] {
		var id, path string
		var width, height int
		if _, err := fmt.Sscanf(line, "ad %s %d %d %s", &id, &width, &height, &path); err != nil {
			t.Fatalf("manifest line %q: %v", line, err)
		}
		status, image := get(t, server, path)
		if status != 200 {
			t.Fatalf("%s: status %d", path, status)
		}
		decoded, err := jpeg.Decode(bytes.NewReader(image))
		if err != nil {
			t.Fatalf("%s: %v", path, err)
		}
		if got := decoded.Bounds().Size(); got.X != width || got.Y != height {
			t.Fatalf("%s: %dx%d, manifest says %dx%d", path, got.X, got.Y, width, height)
		}
	}
}

func TestUnknownAdIsNotFound(t *testing.T) {
	server := httptest.NewServer(handler(nil))
	defer server.Close()
	if status, _ := get(t, server, apiPrefix+"/image/nothing.jpg"); status != http.StatusNotFound {
		t.Fatalf("status = %d", status)
	}
}

func TestSizesHaveOneAdPerPalette(t *testing.T) {
	ads, err := generate()
	if err != nil {
		t.Fatal(err)
	}
	if len(ads) != len(sizes)*len(palettes) {
		t.Fatalf("%d ads", len(ads))
	}
	seen := map[string]bool{}
	for _, a := range ads {
		if seen[a.id] {
			t.Fatalf("duplicate id %s", a.id)
		}
		seen[a.id] = true
	}
}
