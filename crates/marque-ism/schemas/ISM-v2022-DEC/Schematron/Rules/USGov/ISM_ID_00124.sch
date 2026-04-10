<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00124">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00124][Warning] If ISM_USGOV_RESOURCE and
      1. Attribute @ism:ownerProducer does not contain [USA].
      AND
      2. Attribute @ism:disseminationControls contains [RELIDO]
      
      Human Readable: RELIDO is not authorized for non-US portions.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [RELIDO] this rule ensures that attribute @ism:ownerProducer is
    	specified with a value containing [USA].
    </sch:p>
	  <sch:rule id="ISM-ID-00124-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RELIDO'))]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:ownerProducer, ('USA'))" flag="warning" role="warning">
          [ISM-ID-00124][Warning] If ISM_USGOV_RESOURCE and
          1. Attribute @ism:ownerProducer does not contain [USA].
          AND
          2. Attribute @ism:disseminationControls contains [RELIDO]
          
          Human Readable: RELIDO is not authorized for non-US portions.
        </sch:assert>
    </sch:rule>
</sch:pattern>