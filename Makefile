run-wuhu:
	go run wuhu.go

run-ding:
	go run ding.go

build-wuhu:
	GOOS=windows GOARCH=amd64 go build -ldflags="-w -s" -o wuhu.exe wuhu.go

build-ding:
	GOOS=windows GOARCH=amd64 go build -ldflags="-w -s" -o ding.exe ding.go

build: build-wuhu build-ding

clean:
	rm -f wuhu.exe ding.exe

.PHONY: run-wuhu run-ding build-wuhu build-ding build clean 