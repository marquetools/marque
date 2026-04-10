<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="NtkHasCorrespondingDataTwoTokens">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      Abstract template to validate that for an $ISM_USGOV_RESOURCE or
      $ISM_USCUIONLY_RESOURCE, one of two given tokens ($dataType1 or $dataType2) exists in a particular attribute of at
      least one of 
      (a) a portion that contributes to roll-up or 
      (b) the banner, given the existence
      of an ntk:AccessProfile that has an ntk:AccessPolicy value that starts with a given string
      ($uriPrefix).
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      Expected parameters: $ruleId, $policyName, $uriPrefix, $attr, $dataType1, $dataType2,
      $dataTokenList, and $bannerTokenList
   </sch:p>
   <sch:rule id="NtkHasCorrespondingDataTwoTokens-R1" context="ntk:Access//ntk:AccessProfile[ntk:AccessPolicy[starts-with(., $uriPrefix)] and ($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE)]">
      <sch:assert test="index-of($dataTokenList, $dataType1) &gt; 0 or index-of($bannerTokenList, $dataType1) &gt; 0 or index-of($dataTokenList, $dataType2) &gt; 0 or index-of($bannerTokenList, $dataType2) &gt; 0" flag="error" role="error">
         [<sch:value-of select="$ruleId"/>][error] <sch:value-of
            select="$policyName"/> NTK metadata requires that <sch:value-of select="$attr"/> contain
         <sch:value-of select="$dataType1"/> or <sch:value-of select="$dataType2"/> in at least one of (a) a portion that contributes to
         roll-up or (b) the banner.
      </sch:assert>
   </sch:rule>
</sch:pattern>
