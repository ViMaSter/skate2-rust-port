package main

import (
	"bytes"
	"image"
	"image/color"
	"image/jpeg"
	"image/png"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func savePNG(t *testing.T, path string, img image.Image) {
	t.Helper()
	var data bytes.Buffer
	if err := png.Encode(&data, img); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, data.Bytes(), 0644); err != nil {
		t.Fatal(err)
	}
}

func TestPNGDirectoryOrderAndAlpha(t *testing.T) {
	directory := t.TempDir()
	savePNG(t, filepath.Join(directory, "02 transparent.PNG"), image.NewNRGBA(image.Rect(0, 0, 64, 32)))
	red := image.NewRGBA(image.Rect(0, 0, 64, 32))
	fill(red, red.Bounds(), color.RGBA{R: 255, A: 255})
	savePNG(t, filepath.Join(directory, "01 red.png"), red)
	ads, err := loadDirectory(directory)
	if err != nil {
		t.Fatal(err)
	}
	if len(ads) != 2 || ads[0].id != "file-001" || ads[1].id != "file-002" {
		t.Fatalf("unexpected inventory: %v", ads)
	}
	server := httptest.NewServer(handler(ads))
	defer server.Close()
	for index, a := range ads {
		status, body := get(t, server, apiPrefix+"/image/"+a.id+".jpg")
		if status != 200 {
			t.Fatalf("status = %d", status)
		}
		img, err := jpeg.Decode(bytes.NewReader(body))
		if err != nil {
			t.Fatal(err)
		}
		if img.Bounds().Dx() != 64 || img.Bounds().Dy() != 32 {
			t.Fatalf("unexpected dimensions %v", img.Bounds())
		}
		r, g, b, _ := img.At(20, 20).RGBA()
		if r < 65000 || (index == 0 && (g > 500 || b > 500)) || (index == 1 && (g < 65000 || b < 65000)) {
			t.Fatalf("ad %d: unexpected RGB %d %d %d", index, r, g, b)
		}
	}
}

func TestPNGColorModels(t *testing.T) {
	for name, img := range map[string]image.Image{
		"gray":    image.NewGray(image.Rect(0, 0, 16, 16)),
		"rgba16":  image.NewNRGBA64(image.Rect(0, 0, 16, 16)),
		"indexed": image.NewPaletted(image.Rect(0, 0, 16, 16), color.Palette{color.White}),
	} {
		t.Run(name, func(t *testing.T) {
			path := filepath.Join(t.TempDir(), "ad.png")
			savePNG(t, path, img)
			a, err := loadPNG(path)
			if err != nil {
				t.Fatal(err)
			}
			img, err := jpeg.Decode(bytes.NewReader(a.jpeg))
			if err != nil {
				t.Fatal(err)
			}
			if _, ok := img.(*image.YCbCr); !ok {
				t.Fatalf("client requires RGB JPEG; got %T", img)
			}
		})
	}
}

func TestPNGDirectoryRejectsInvalidInput(t *testing.T) {
	for _, kind := range []string{"empty", "corrupt", "oversize", "too-many", "symlink"} {
		t.Run(kind, func(t *testing.T) {
			directory := t.TempDir()
			path := filepath.Join(directory, "ad.png")
			switch kind {
			case "corrupt":
				if err := os.WriteFile(path, []byte("not png"), 0644); err != nil {
					t.Fatal(err)
				}
			case "oversize":
				savePNG(t, path, image.NewRGBA(image.Rect(0, 0, maxSide+1, 1)))
			case "too-many":
				for index := 0; index <= maxAds; index++ {
					savePNG(t, filepath.Join(directory, strings.Repeat("a", index+1)+".png"), image.NewRGBA(image.Rect(0, 0, 1, 1)))
				}
			case "symlink":
				if err := os.Symlink("missing.png", path); err != nil {
					t.Fatal(err)
				}
			}
			if _, err := loadDirectory(directory); err == nil {
				t.Fatal("expected an explicit load error")
			}
		})
	}
}

func TestExamplesFormFourFrameRotations(t *testing.T) {
	directory := t.TempDir()
	if err := writeExamples(directory); err != nil {
		t.Fatal(err)
	}
	ads, err := loadDirectory(directory)
	if err != nil {
		t.Fatal(err)
	}
	counts := map[size]int{}
	for _, a := range ads {
		counts[size{a.width, a.height}]++
	}
	if len(counts) != len(sizes) {
		t.Fatalf("got %d sizes", len(counts))
	}
	for _, s := range sizes {
		if counts[s] != 4 {
			t.Fatalf("%v: %d rotation frames", s, counts[s])
		}
	}
	path := filepath.Join(directory, "01-magenta-256x128.png")
	before, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := writeExamples(directory); err == nil {
		t.Fatal("must not overwrite existing examples")
	}
	after, err := os.ReadFile(path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatal("existing example changed")
	}
}
