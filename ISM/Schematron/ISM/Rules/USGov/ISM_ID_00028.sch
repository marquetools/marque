<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00028">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00028][Error] If ISM_USGOV_RESOURCE and attribute 
      @ism:disseminationControls contains the name token [OC] or [EYES],
      then attribute @ism:classification must have a value of [TS], [S], or [C].
      Human Readable: Portions marked for ORCON or EYES ONLY dissemination 
      in a USA document must be CONFIDENTIAL, SECRET, or TOP SECRET.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [OC] or [EYES] this rule ensures that attribute
    	@ism:classification is specified with a value of [C], [S], or [TS].
    </sch:p>
	  <sch:rule id="ISM-ID-00028-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC', 'EYES'))]">
        <sch:assert test="@ism:classification=('TS', 'S', 'C')" flag="error" role="error">
            [ISM-ID-00028][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls contains the name token [OC] or [EYES],
            then attribute @ism:classification must have a value of [TS], [S], or [C].
            Human Readable: Portions marked for ORCON or EYES ONLY dissemination 
            in a USA document must be CONFIDENTIAL, SECRET, or TOP SECRET.
        </sch:assert>
    </sch:rule>
</sch:pattern>