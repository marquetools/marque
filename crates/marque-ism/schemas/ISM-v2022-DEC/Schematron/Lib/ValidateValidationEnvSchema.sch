<?xml version="1.0" encoding="utf-8"?>
<!--
    This abstract pattern checks to see the version of a Schema is greater than or equal to a passed in parameter.

    $MinVersion             := the version that SpecToCheck must be equal to or greater than.
    $SpecToCheck            := Name the spec whose version in the infrastructure is being checked. 
    $pathToDocument         := Relative path to the document xsd that has ther version string
    $RuleID                 := The number of the rule in the concrete file. 

-->
<sch:pattern xmlns:ism="urn:us:gov:ic:ism" xmlns:sch="http://purl.oclc.org/dsdl/schematron"
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform" abstract="true" id="ValidateValidationEnvSchema">
    
    <sch:p class="codeDesc" ism:classification="U" ism:ownerProducer="USA">
        This abstract pattern checks to see if the validation environment has at least the version / revision of the
        Schema as of the writing of this specification. 
        The calling rule must pass in $MinVersion, $SpecToCheck, $pathToDocument, $RuleID.</sch:p>
    
    <sch:rule id="ValidateValidationEnvSchema-R1" context="/">
        <sch:assert test="document($pathToDocument)//xsd:schema//@version castable as xs:double 
            and document($pathToDocument)//xsd:schema//@version &gt;= $MinVersion" flag="error" role="error"> 
            [<sch:value-of select="$RuleID"/>][Error] Version [ <sch:value-of select="document($pathToDocument)//xsd:schema//@version"/> ] of <sch:value-of select="$SpecToCheck"/> found; 
            Version [<sch:value-of select="$MinVersion"/>] or later is required. The latest version of <sch:value-of select="$SpecToCheck"/> 
            is not being used in the validation infrastructure. Regardless of the version indicated on the instance document, 
            the validation infrastructure needs to use a version of <sch:value-of select="$SpecToCheck"/> that is
            version [<sch:value-of select="$MinVersion"/>] or later. NOTE: This is not an error of the instance
            document but of the validation environment itself. The incorrect value was found in <sch:value-of select="document-uri(document($pathToDocument))"/>
        </sch:assert>
    </sch:rule>
</sch:pattern>