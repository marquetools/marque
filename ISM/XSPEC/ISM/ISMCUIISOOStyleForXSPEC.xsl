<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:ism="urn:us:gov:ic:ism" version="2.0">

	<xsl:import href="../../XSL/ISM/IC-ISM-ISOO-Rendering.xsl"/>

	<xsl:output method="text"/>
	
	
	
	
	<!-- 
	<xsl:apply-templates select="." mode="ism:portionmark"/>
	<xsl:when test="local-name($test-element)='banner'">
					<xsl:apply-templates select="$test-element/parent::*/sampleAttributes" mode="ism:banner"/>
				</xsl:when>
				<xsl:when test="local-name($test-element)='portion'">
					<xsl:apply-templates select="$test-element/parent::*/sampleAttributes"
						mode="ism:portionmark"/>
				</xsl:when>
				<xsl:when test="local-name($test-element)=$authority-element">
					<xsl:apply-templates select="$test-element/parent::*/sampleAttributes"
						mode="ism:authority"/>
				</xsl:when>
	-->

</xsl:stylesheet>
