<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<!--
    This abstract pattern checks to see if an attribute of an element exists in a list.

    $context     := the context in which the searchValue exists
    $searchTerm  := the value which you want to verify is in the list
    $list        := the list in which to search for the searchValue
    $errMsg      := the error message text to display when the assertion fails
    
    Example usage:
    <sch:pattern is-a="ValidateValueExistenceInList" id="IRM_ID_00027" xmlns:sch="http://purl.oclc.org/dsdl/schematron">  
        <sch:param name="context" value="//irm:CountryCode[@irm:vocabulary='FIPS']"/>
        <sch:param name="searchTerm" value="."/>
        <sch:param name="list" value="$coverageFipsDigraphList"/>
        <sch:param name="errMsg" value="'
            [IRM-ID-00027][Error]
            If element CountryCode has attribute vocabulary specified as FIPS
            the element value must be in CVEnumIRMCoverageFIPSDigraph.xml.
        '"/>
    </sch:pattern>
    
    Note: $iso4217TrigraphList is defined in the main document, IRM_XML.xml.
--><sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="ValidateValueExistenceInList">
    
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">This abstract pattern checks to see if an attribute of an element exists
        in a list. The calling rule must pass $context, $searchTerm, $list, $errMsg.</sch:p>

    <sch:rule id="ValidateValueExistenceInList-R1" context="$context"> 
        <sch:assert test="some $token in $list satisfies $token = $searchTerm" flag="error" role="error">
            <sch:value-of select="$errMsg"/>
        </sch:assert>
    </sch:rule>
</sch:pattern>