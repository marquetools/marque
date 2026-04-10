<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00016">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00016][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:classification has a value of [U], then attributes @ism:classificationReason,
        @ism:classifiedBy, @ism:derivativelyClassifiedBy, @ism:declassDate, @ism:declassEvent, 
        @ism:declassException, @ism:derivedFrom, @ism:SARIdentifier, or @ism:SCIcontrols must not be specified.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:classification specified with a value of [U] this rule ensures that NONE of the following attributes 
    	are specified: @ism:classifiedBy, @ism:declassDate, @ism:declassEvent, @ism:declassException,
    	@ism:derivativelyClassifiedBy, @ism:derivedFrom, @ism:SARIdentifier, or @ism:SCIcontrols. 
    </sch:p>
	  <sch:rule id="ISM-ID-00016-R1" context="*[$ISM_USGOV_RESOURCE and @ism:classification='U']">
        <sch:assert test="not(@ism:classificationReason or @ism:classifiedBy or @ism:declassDate or @ism:declassEvent or @ism:declassException or @ism:derivativelyClassifiedBy or @ism:derivedFrom or @ism:SARIdentifier or @ism:SCIcontrols)" flag="error" role="error">
            [ISM-ID-00016][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:classification has a value of [U], then attributes @ism:classificationReason,
            @ism:classifiedBy, @ism:derivativelyClassifiedBy, @ism:declassDate, @ism:declassEvent, 
            @ism:declassException, @ism:derivedFrom, @ism:SARIdentifier, or @ism:SCIcontrols must not be specified.
        </sch:assert>
    </sch:rule>
</sch:pattern>