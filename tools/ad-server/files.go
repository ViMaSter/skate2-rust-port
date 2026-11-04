package main

import (
	"bytes"
	"fmt"
	"image"
	"image/draw"
	"image/jpeg"
	"image/png"
	"io"
	"log"
	"os"
	"path/filepath"
	"strings"
)

const (
	maxAds       = 64
	maxSide      = 2048
	maxPNGBytes  = 16 * 1024 * 1024
	maxJPEGBytes = 4 * 1024 * 1024
)

func loadDirectory(directory string) ([]ad, error) {
	entries, err := os.ReadDir(directory)
	if err != nil {
		return nil, err
	}
	var ads []ad
	for _, entry := range entries {
		if !strings.EqualFold(filepath.Ext(entry.Name()), ".png") {
			continue
		}
		if !entry.Type().IsRegular() {
			return nil, fmt.Errorf("%s: PNG must be a regular file, not a directory or symlink", entry.Name())
		}
		if len(ads) == maxAds {
			return nil, fmt.Errorf("%s: more than %d PNG ads", directory, maxAds)
		}
		a, err := loadPNG(filepath.Join(directory, entry.Name()))
		if err != nil {
			return nil, fmt.Errorf("%s: %w", entry.Name(), err)
		}
		// ReadDir sorts by filename; IDs stay safe in the wire format even with spaces in names.
		a.id = fmt.Sprintf("file-%03d", len(ads)+1)
		ads = append(ads, a)
		log.Printf("%s = %s (%dx%d)", a.id, entry.Name(), a.width, a.height)
	}
	if len(ads) == 0 {
		return nil, fmt.Errorf("%s: no PNG ads found", directory)
	}
	return ads, nil
}

func loadPNG(path string) (ad, error) {
	file, err := os.Open(path)
	if err != nil {
		return ad{}, err
	}
	defer file.Close()
	data, err := io.ReadAll(io.LimitReader(file, maxPNGBytes+1))
	if err != nil {
		return ad{}, err
	}
	if len(data) > maxPNGBytes {
		return ad{}, fmt.Errorf("PNG exceeds %d MiB", maxPNGBytes/(1024*1024))
	}
	config, err := png.DecodeConfig(bytes.NewReader(data))
	if err != nil {
		return ad{}, err
	}
	if config.Width < 1 || config.Height < 1 || config.Width > maxSide || config.Height > maxSide {
		return ad{}, fmt.Errorf("dimensions %dx%d must be within 1..%d", config.Width, config.Height, maxSide)
	}
	source, err := png.Decode(bytes.NewReader(data))
	if err != nil {
		return ad{}, err
	}
	// The client consumes RGB JPEGs. Composite alpha over white rather than discarding it.
	opaque := image.NewRGBA(source.Bounds())
	draw.Draw(opaque, opaque.Bounds(), image.White, image.Point{}, draw.Src)
	draw.Draw(opaque, opaque.Bounds(), source, source.Bounds().Min, draw.Over)
	var encoded bytes.Buffer
	if err := jpeg.Encode(&encoded, opaque, &jpeg.Options{Quality: 95}); err != nil {
		return ad{}, err
	}
	if encoded.Len() > maxJPEGBytes {
		return ad{}, fmt.Errorf("converted JPEG exceeds %d MiB; use a smaller PNG", maxJPEGBytes/(1024*1024))
	}
	return ad{width: config.Width, height: config.Height, jpeg: encoded.Bytes()}, nil
}

func writeExamples(directory string) error {
	if err := os.MkdirAll(directory, 0755); err != nil {
		return err
	}
	for _, s := range sizes {
		for index, p := range palettes {
			path := filepath.Join(directory, fmt.Sprintf("%02d-%s-%dx%d.png", index+1, p.name, s.width, s.height))
			var encoded bytes.Buffer
			if err := png.Encode(&encoded, render(s, index, p)); err != nil {
				return err
			}
			file, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0644)
			if err != nil {
				return err
			}
			_, writeErr := file.Write(encoded.Bytes())
			closeErr := file.Close()
			if writeErr != nil {
				return writeErr
			}
			if closeErr != nil {
				return closeErr
			}
			log.Printf("wrote %s", path)
		}
	}
	return nil
}
