<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00017">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00017][Error] If ISM_NSI_EO_APPLIES and attribute 
        @ism:classifiedBy is specified, then attribute @ism:classificationReason must be specified.         
        Human Readable: Documents under E.O. 13526 containing Originally Classified data require a
        classification reason to be identified.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	If ISM_NSI_EO_APPLIES, for each element which specifies attribute @ism:classifiedBy, 
    	this rule ensures that attribute @ism:classificationReason is specified.
    </sch:p>
	  <sch:rule id="ISM-ID-00017-R1" context="*[$ISM_NSI_EO_APPLIES and @ism:classifiedBy]">
        <sch:assert test="@ism:classificationReason" flag="error" role="error">
            [ISM-ID-00017][Error] If ISM_NSI_EO_APPLIES and attribute 
            @ism:classifiedBy is specified, then attribute @ism:classificationReason must be specified.         
            Human Readable: Documents under E.O. 13526 containing Originally Classified data require a
            classification reason to be identified.
        </sch:assert>
    </sch:rule>
</sch:pattern>