<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:xs="http://www.w3.org/2001/XMLSchema"
    xmlns:xd="http://www.oxygenxml.com/ns/doc/xsl"
    xmlns:util="urn:us:gov:ic:ism-rollup:xsl:util"
    exclude-result-prefixes="xs xd"
    version="2.0">

    <xsl:import href="ISM-Rollup.xsl"/>

    <xd:doc scope="stylesheet">
        <xd:desc>
            <xd:p><xd:b>Created on:</xd:b> Jul 26, 2020</xd:p>
            <xd:p><xd:b>Author:</xd:b> bob</xd:p>
            <xd:p>Overrides the resourceElement so that it has to be calculated for every run.
            MUCH slower but works for XSpec and XSpec fails many tests without it.</xd:p>
        </xd:desc>
    </xd:doc>
    <xsl:variable name="resourceElement" select="/parent::*"/>
    
    
</xsl:stylesheet>